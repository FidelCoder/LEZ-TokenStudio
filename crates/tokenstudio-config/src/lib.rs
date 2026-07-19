use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use attestation_types::{Digest32, GateContext, ProgramOwner};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const TOKEN_CONFIG_VERSION: u16 = 1;
pub const GATE_CONFIG_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenConfig {
    pub schema_version: u16,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    #[serde(with = "attestation_types::serde_digest_hex")]
    pub definition_account_id: Digest32,
    #[serde(with = "attestation_types::serde_program_owner_hex")]
    pub token_program_owner: ProgramOwner,
    pub definition_account: String,
    pub supply_account: String,
    pub issuer_account: String,
    pub total_supply: u128,
}

impl TokenConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != TOKEN_CONFIG_VERSION {
            return Err(ConfigError::Invalid(format!(
                "unsupported token config version {}; expected {TOKEN_CONFIG_VERSION}",
                self.schema_version
            )));
        }
        validate_label("token name", &self.name, 64)?;
        validate_symbol(&self.symbol)?;
        if self.decimals > 18 {
            return Err(ConfigError::Invalid(
                "token decimals must be between 0 and 18".to_owned(),
            ));
        }
        if self.definition_account_id == [0; 32] {
            return Err(ConfigError::Invalid(
                "token definition account id cannot be all zero".to_owned(),
            ));
        }
        if self.token_program_owner == [0; 8] {
            return Err(ConfigError::Invalid(
                "token program owner cannot be all zero".to_owned(),
            ));
        }
        validate_label("definition account", &self.definition_account, 256)?;
        validate_label("supply account", &self.supply_account, 256)?;
        validate_label("issuer account", &self.issuer_account, 256)?;
        if self.total_supply == 0 {
            return Err(ConfigError::Invalid(
                "token total supply must be greater than zero".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateConfig {
    pub schema_version: u16,
    pub token: TokenConfig,
    pub context: GateContext,
}

impl GateConfig {
    pub fn new(token: TokenConfig, context: GateContext) -> Result<Self, ConfigError> {
        let config = Self {
            schema_version: GATE_CONFIG_VERSION,
            token,
            context,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.schema_version != GATE_CONFIG_VERSION {
            return Err(ConfigError::Invalid(format!(
                "unsupported gate config version {}; expected {GATE_CONFIG_VERSION}",
                self.schema_version
            )));
        }
        self.token.validate()?;
        validate_label("application id", &self.context.application_id, 128)?;
        validate_label("gate id", &self.context.gate_id, 128)?;
        validate_label("verifier id", &self.context.verifier_id, 256)?;
        if self.context.threshold == 0 {
            return Err(ConfigError::Invalid(
                "gate threshold must be greater than zero".to_owned(),
            ));
        }
        if self.context.token_program_owner != self.token.token_program_owner {
            return Err(ConfigError::Invalid(
                "gate token program owner does not match token config".to_owned(),
            ));
        }
        if self.context.token_definition_id != self.token.definition_account_id {
            return Err(ConfigError::Invalid(
                "gate token definition id does not match token config".to_owned(),
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn context_hash(&self) -> Digest32 {
        self.context.context_hash()
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("{0}")]
    Invalid(String),
    #[error("failed to read or write config: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid config JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("LEZ wallet command failed with status {0}")]
    WalletFailed(ExitStatus),
}

pub fn read_token_config(path: impl AsRef<Path>) -> Result<TokenConfig, ConfigError> {
    let config = serde_json::from_slice::<TokenConfig>(&fs::read(path)?)?;
    config.validate()?;
    Ok(config)
}

pub fn read_gate_config(path: impl AsRef<Path>) -> Result<GateConfig, ConfigError> {
    let config = serde_json::from_slice::<GateConfig>(&fs::read(path)?)?;
    config.validate()?;
    Ok(config)
}

pub fn write_token_config(path: impl AsRef<Path>, config: &TokenConfig) -> Result<(), ConfigError> {
    config.validate()?;
    write_json(path.as_ref(), config)
}

pub fn write_gate_config(path: impl AsRef<Path>, config: &GateConfig) -> Result<(), ConfigError> {
    config.validate()?;
    write_json(path.as_ref(), config)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WalletInvocation {
    pub binary: PathBuf,
    pub args: Vec<String>,
}

impl WalletInvocation {
    #[must_use]
    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.binary);
        command.args(&self.args);
        command
    }

    pub fn run(&self) -> Result<(), ConfigError> {
        let status = self.command().status()?;
        if status.success() {
            Ok(())
        } else {
            Err(ConfigError::WalletFailed(status))
        }
    }

    #[must_use]
    pub fn display(&self) -> String {
        std::iter::once(self.binary.as_os_str())
            .chain(self.args.iter().map(OsStr::new))
            .map(|part| format!("{part:?}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

pub fn create_token_invocation(
    wallet_binary: impl Into<PathBuf>,
    token: &TokenConfig,
) -> WalletInvocation {
    WalletInvocation {
        binary: wallet_binary.into(),
        args: vec![
            "token".to_owned(),
            "new".to_owned(),
            "--definition-account-id".to_owned(),
            token.definition_account.clone(),
            "--supply-account-id".to_owned(),
            token.supply_account.clone(),
            "--name".to_owned(),
            token.name.clone(),
            "--total-supply".to_owned(),
            token.total_supply.to_string(),
        ],
    }
}

pub fn mint_token_invocation(
    wallet_binary: impl Into<PathBuf>,
    token: &TokenConfig,
    holder: &str,
    amount: u128,
) -> Result<WalletInvocation, ConfigError> {
    validate_label("holder account", holder, 256)?;
    if amount == 0 {
        return Err(ConfigError::Invalid(
            "mint amount must be greater than zero".to_owned(),
        ));
    }
    Ok(WalletInvocation {
        binary: wallet_binary.into(),
        args: vec![
            "token".to_owned(),
            "mint".to_owned(),
            "--definition".to_owned(),
            token.definition_account.clone(),
            "--holder".to_owned(),
            holder.to_owned(),
            "--amount".to_owned(),
            amount.to_string(),
        ],
    })
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), ConfigError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut json = serde_json::to_vec_pretty(value)?;
    json.push(b'\n');
    fs::write(path, json)?;
    Ok(())
}

fn validate_label(field: &str, value: &str, max_len: usize) -> Result<(), ConfigError> {
    if value.trim().is_empty() {
        return Err(ConfigError::Invalid(format!("{field} cannot be empty")));
    }
    if value.len() > max_len {
        return Err(ConfigError::Invalid(format!(
            "{field} cannot exceed {max_len} bytes"
        )));
    }
    Ok(())
}

fn validate_symbol(symbol: &str) -> Result<(), ConfigError> {
    if !(2..=12).contains(&symbol.len()) {
        return Err(ConfigError::Invalid(
            "token symbol must contain 2 to 12 ASCII characters".to_owned(),
        ));
    }
    if !symbol
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit())
    {
        return Err(ConfigError::Invalid(
            "token symbol may contain only uppercase ASCII letters and digits".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::{digest_from_hex, program_owner_from_hex};

    fn token() -> TokenConfig {
        TokenConfig {
            schema_version: TOKEN_CONFIG_VERSION,
            name: "Founders Token".to_owned(),
            symbol: "FNDR".to_owned(),
            decimals: 0,
            definition_account_id: [9; 32],
            token_program_owner: [7; 8],
            definition_account: "Private/founders-definition".to_owned(),
            supply_account: "Private/founders-supply".to_owned(),
            issuer_account: "Private/founders-issuer".to_owned(),
            total_supply: 1_000_000,
        }
    }

    fn gate() -> GateConfig {
        let token = token();
        GateConfig::new(
            token.clone(),
            GateContext {
                application_id: "tokenstudio".to_owned(),
                gate_id: "founders-chat".to_owned(),
                token_program_owner: token.token_program_owner,
                token_definition_id: token.definition_account_id,
                threshold: 100,
                verifier_id: "logos-chat:founders".to_owned(),
                expires_at_unix_ms: Some(1_900_000_000_000),
            },
        )
        .unwrap()
    }

    #[test]
    fn gate_round_trip_preserves_context_hash() {
        let gate = gate();
        let json = serde_json::to_string_pretty(&gate).unwrap();
        let decoded: GateConfig = serde_json::from_str(&json).unwrap();

        decoded.validate().unwrap();
        assert_eq!(decoded.context_hash(), gate.context_hash());
    }

    #[test]
    fn rejects_gate_for_different_asset() {
        let mut gate = gate();
        gate.context.token_definition_id[0] ^= 1;

        assert!(gate
            .validate()
            .unwrap_err()
            .to_string()
            .contains("definition"));
    }

    #[test]
    fn wallet_commands_match_official_cli() {
        let token = token();
        let create = create_token_invocation("wallet", &token);
        assert_eq!(
            create.args,
            [
                "token",
                "new",
                "--definition-account-id",
                "Private/founders-definition",
                "--supply-account-id",
                "Private/founders-supply",
                "--name",
                "Founders Token",
                "--total-supply",
                "1000000",
            ]
        );

        let mint = mint_token_invocation("wallet", &token, "Private/founders-holder", 250).unwrap();
        assert_eq!(
            mint.args,
            [
                "token",
                "mint",
                "--definition",
                "Private/founders-definition",
                "--holder",
                "Private/founders-holder",
                "--amount",
                "250",
            ]
        );
    }

    #[test]
    fn example_hex_values_are_valid() {
        assert_eq!(digest_from_hex(&"11".repeat(32)).unwrap(), [0x11; 32]);
        assert_ne!(
            program_owner_from_hex(
                "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
            )
            .unwrap(),
            [0; 8]
        );
    }
}

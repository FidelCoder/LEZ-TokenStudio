use std::{
    path::PathBuf,
    process,
    time::{SystemTime, UNIX_EPOCH},
};

use attestation_types::{digest_from_hex, digest_to_hex, program_owner_from_hex, GateContext};
use clap::{Args, Parser, Subcommand};
use tokenstudio_config::{
    create_token_invocation, mint_token_invocation, read_gate_config, read_token_config,
    write_gate_config, write_token_config, GateConfig, TokenConfig, TOKEN_CONFIG_VERSION,
};

#[derive(Debug, Parser)]
#[command(
    name = "proofgate",
    version,
    about = "No-code private token gates for Logos"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Version,
    ContextHash(ContextArgs),
    Token {
        #[command(subcommand)]
        command: TokenCommand,
    },
    Gate {
        #[command(subcommand)]
        command: GateCommand,
    },
}

#[derive(Debug, Subcommand)]
enum TokenCommand {
    Select(TokenConfigArgs),
    Create(TokenCreateArgs),
    Mint(TokenMintArgs),
}

#[derive(Debug, Subcommand)]
enum GateCommand {
    Init(GateInitArgs),
    Hash {
        #[arg(long)]
        gate: PathBuf,
    },
    Show {
        #[arg(long)]
        gate: PathBuf,
    },
}

#[derive(Clone, Debug, Args)]
struct TokenConfigArgs {
    #[arg(long)]
    name: String,
    #[arg(long)]
    symbol: String,
    #[arg(long, default_value_t = 0)]
    decimals: u8,
    #[arg(long)]
    definition_account_id_hex: String,
    #[arg(long)]
    token_program_owner_hex: String,
    #[arg(long)]
    definition_account: String,
    #[arg(long)]
    supply_account: String,
    #[arg(long)]
    issuer_account: String,
    #[arg(long)]
    total_supply: u128,
    #[arg(long)]
    output: PathBuf,
}

impl TokenConfigArgs {
    fn config(&self) -> Result<TokenConfig, String> {
        Ok(TokenConfig {
            schema_version: TOKEN_CONFIG_VERSION,
            name: self.name.clone(),
            symbol: self.symbol.clone(),
            decimals: self.decimals,
            definition_account_id: digest_from_hex(&self.definition_account_id_hex)
                .map_err(|error| format!("invalid --definition-account-id-hex: {error}"))?,
            token_program_owner: program_owner_from_hex(&self.token_program_owner_hex)
                .map_err(|error| format!("invalid --token-program-owner-hex: {error}"))?,
            definition_account: self.definition_account.clone(),
            supply_account: self.supply_account.clone(),
            issuer_account: self.issuer_account.clone(),
            total_supply: self.total_supply,
        })
    }
}

#[derive(Debug, Args)]
struct TokenCreateArgs {
    #[command(flatten)]
    token: TokenConfigArgs,
    #[arg(long, default_value = "wallet")]
    wallet_binary: PathBuf,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct TokenMintArgs {
    #[arg(long)]
    token: PathBuf,
    #[arg(long)]
    holder: String,
    #[arg(long)]
    amount: u128,
    #[arg(long, default_value = "wallet")]
    wallet_binary: PathBuf,
    #[arg(long)]
    dry_run: bool,
}

#[derive(Debug, Args)]
struct GateInitArgs {
    #[arg(long)]
    token: PathBuf,
    #[arg(long)]
    application_id: String,
    #[arg(long)]
    gate_id: String,
    #[arg(long)]
    threshold: u128,
    #[arg(long)]
    verifier_id: String,
    #[arg(long)]
    expires_at_unix_ms: Option<u64>,
    #[arg(long)]
    output: PathBuf,
}

#[derive(Debug, Args)]
struct ContextArgs {
    #[arg(long)]
    application_id: String,
    #[arg(long)]
    gate_id: String,
    #[arg(long)]
    token_owner_hex: String,
    #[arg(long)]
    token_definition_id_hex: String,
    #[arg(long)]
    threshold: u128,
    #[arg(long)]
    verifier_id: String,
    #[arg(long)]
    expires_at_unix_ms: Option<u64>,
}

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error}");
        process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Version => {
            println!("{}", env!("CARGO_PKG_VERSION"));
        }
        Command::ContextHash(args) => {
            println!(
                "{}",
                digest_to_hex(&context_from_args(args)?.context_hash())
            );
        }
        Command::Token { command } => run_token_command(command)?,
        Command::Gate { command } => run_gate_command(command)?,
    }
    Ok(())
}

fn run_token_command(command: TokenCommand) -> Result<(), String> {
    match command {
        TokenCommand::Select(args) => {
            let config = args.config()?;
            write_token_config(&args.output, &config).map_err(|error| error.to_string())?;
            println!("Token config written to {}", args.output.display());
        }
        TokenCommand::Create(args) => {
            let config = args.token.config()?;
            config.validate().map_err(|error| error.to_string())?;
            let invocation = create_token_invocation(args.wallet_binary, &config);
            println!("LEZ wallet command: {}", invocation.display());
            if !args.dry_run {
                invocation.run().map_err(|error| error.to_string())?;
            }
            write_token_config(&args.token.output, &config).map_err(|error| error.to_string())?;
            println!("Token config written to {}", args.token.output.display());
        }
        TokenCommand::Mint(args) => {
            let token = read_token_config(&args.token).map_err(|error| error.to_string())?;
            let invocation =
                mint_token_invocation(args.wallet_binary, &token, &args.holder, args.amount)
                    .map_err(|error| error.to_string())?;
            println!("LEZ wallet command: {}", invocation.display());
            if !args.dry_run {
                invocation.run().map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn run_gate_command(command: GateCommand) -> Result<(), String> {
    match command {
        GateCommand::Init(args) => {
            if let Some(expiry) = args.expires_at_unix_ms {
                if expiry <= now_unix_ms()? {
                    return Err("--expires-at-unix-ms must be in the future".to_owned());
                }
            }
            let token = read_token_config(&args.token).map_err(|error| error.to_string())?;
            let context = GateContext {
                application_id: args.application_id,
                gate_id: args.gate_id,
                token_program_owner: token.token_program_owner,
                token_definition_id: token.definition_account_id,
                threshold: args.threshold,
                verifier_id: args.verifier_id,
                expires_at_unix_ms: args.expires_at_unix_ms,
            };
            let config = GateConfig::new(token, context).map_err(|error| error.to_string())?;
            write_gate_config(&args.output, &config).map_err(|error| error.to_string())?;
            println!("Gate config written to {}", args.output.display());
            println!("Context hash: {}", digest_to_hex(&config.context_hash()));
        }
        GateCommand::Hash { gate } => {
            let config = read_gate_config(gate).map_err(|error| error.to_string())?;
            println!("{}", digest_to_hex(&config.context_hash()));
        }
        GateCommand::Show { gate } => {
            let config = read_gate_config(gate).map_err(|error| error.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&config).map_err(|error| error.to_string())?
            );
        }
    }
    Ok(())
}

fn context_from_args(args: ContextArgs) -> Result<GateContext, String> {
    Ok(GateContext {
        application_id: args.application_id,
        gate_id: args.gate_id,
        token_program_owner: program_owner_from_hex(&args.token_owner_hex)
            .map_err(|error| format!("invalid --token-owner-hex: {error}"))?,
        token_definition_id: digest_from_hex(&args.token_definition_id_hex)
            .map_err(|error| format!("invalid --token-definition-id-hex: {error}"))?,
        threshold: args.threshold,
        verifier_id: args.verifier_id,
        expires_at_unix_ms: args.expires_at_unix_ms,
    })
}

fn now_unix_ms() -> Result<u64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "system clock is before Unix epoch".to_owned())?;
    u64::try_from(duration.as_millis()).map_err(|_| "current time exceeds u64".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nested_gate_hash_command() {
        let cli = Cli::try_parse_from([
            "proofgate",
            "gate",
            "hash",
            "--gate",
            "examples/gates/founders.json",
        ])
        .unwrap();

        assert!(matches!(
            cli.command,
            Command::Gate {
                command: GateCommand::Hash { .. }
            }
        ));
    }

    #[test]
    fn context_hash_requires_specific_token_definition() {
        let error = Cli::try_parse_from([
            "proofgate",
            "context-hash",
            "--application-id",
            "tokenstudio",
            "--gate-id",
            "founders",
            "--token-owner-hex",
            &"11".repeat(32),
            "--threshold",
            "100",
            "--verifier-id",
            "logos-chat:founders",
        ])
        .unwrap_err();

        assert!(error.to_string().contains("token-definition-id-hex"));
    }

    #[test]
    fn gate_config_version_is_current() {
        assert_eq!(tokenstudio_config::GATE_CONFIG_VERSION, 1);
    }
}

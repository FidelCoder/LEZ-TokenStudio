use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
};

use attestation_circuit::BalanceAttestationWitness;
use attestation_types::{Digest32, PublicKey32};
use jsonrpsee::{core::client::ClientT as _, http_client::HttpClientBuilder, rpc_params};
use lez_compat::{LezAccount, MembershipProof};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use tokenstudio_config::GateConfig;

use crate::{prove_request, ProverError, Risc0AttestationProof};

type SequencerMembershipProof = Option<(usize, Vec<Digest32>)>;
type SequencerMembershipResponse = (Vec<Option<(usize, Vec<Digest32>)>>, Digest32);

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateAccountSnapshot {
    pub account: LezAccount,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProverInputFile {
    pub account: LezAccount,
    pub membership_proof: MembershipProof,
    pub commitment_root: Digest32,
}

impl ProverInputFile {
    pub fn validate_membership(&self) -> Result<(), InputError> {
        let commitment = self.account.commitment();
        if self
            .membership_proof
            .verifies(&commitment, &self.commitment_root)
        {
            Ok(())
        } else {
            Err(InputError::MembershipRootMismatch)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProofRequest {
    pub gate: GateConfig,
    pub input: ProverInputFile,
    pub presenter_public_key: PublicKey32,
    pub issued_at_unix_ms: u64,
}

impl ProofRequest {
    pub fn witness(&self) -> Result<BalanceAttestationWitness, InputError> {
        self.gate
            .validate()
            .map_err(|error| InputError::InvalidGate(error.to_string()))?;
        self.input.validate_membership()?;

        Ok(BalanceAttestationWitness {
            account: self.input.account.clone(),
            membership_proof: self.input.membership_proof.clone(),
            commitment_root: self.input.commitment_root,
            gate_context: self.gate.context.clone(),
            presenter_public_key: self.presenter_public_key,
            issued_at_unix_ms: self.issued_at_unix_ms,
        })
    }
}

#[derive(Debug, Error)]
pub enum InputError {
    #[error("invalid gate configuration: {0}")]
    InvalidGate(String),
    #[error("failed to read or write prover data: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid prover JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("wallet account mention must use Private/<base58-account-id>")]
    InvalidPrivateAccountMention,
    #[error("wallet account id is not valid base58")]
    InvalidAccountBase58,
    #[error("wallet account id must decode to exactly 32 bytes; got {0}")]
    InvalidAccountIdLength(usize),
    #[error("wallet program owner is not valid base58")]
    InvalidProgramOwnerBase58,
    #[error("wallet program owner must decode to exactly 32 bytes; got {0}")]
    InvalidProgramOwnerLength(usize),
    #[error("wallet account data is not valid hexadecimal")]
    InvalidAccountDataHex,
    #[error("LEZ wallet account command failed with status {0}")]
    WalletCommandFailed(ExitStatus),
    #[error("LEZ wallet output did not contain account JSON")]
    MissingWalletJson,
    #[error("failed to create sequencer RPC client: {0}")]
    RpcClient(String),
    #[error("sequencer membership proof request failed: {0}")]
    RpcRequest(String),
    #[error("sequencer returned {0} membership proof entries; expected exactly one")]
    UnexpectedProofCount(usize),
    #[error("sequencer does not contain the private account commitment")]
    MissingMembershipProof,
    #[error("sequencer membership proof does not match its returned commitment root")]
    MembershipRootMismatch,
}

pub async fn prove_live(
    gate: GateConfig,
    snapshot: PrivateAccountSnapshot,
    sequencer_url: &str,
    presenter_public_key: PublicKey32,
    issued_at_unix_ms: u64,
) -> Result<Risc0AttestationProof, ProverError> {
    let input = fetch_sequencer_input(&snapshot.account, sequencer_url).await?;
    prove_request(&ProofRequest {
        gate,
        input,
        presenter_public_key,
        issued_at_unix_ms,
    })
}

pub async fn fetch_sequencer_commitment_root(sequencer_url: &str) -> Result<Digest32, InputError> {
    Ok(fetch_sequencer_input(&LezAccount::default(), sequencer_url)
        .await?
        .commitment_root)
}

pub async fn fetch_sequencer_input(
    account: &LezAccount,
    sequencer_url: &str,
) -> Result<ProverInputFile, InputError> {
    let commitment = account.commitment();
    let client = HttpClientBuilder::default()
        .build(sequencer_url)
        .map_err(|error| InputError::RpcClient(error.to_string()))?;

    let current_response: Result<SequencerMembershipResponse, _> = client
        .request(
            "getProofsAndRoot",
            rpc_params![vec![*commitment.as_bytes()]],
        )
        .await;

    let (membership_proof, commitment_root) = match current_response {
        Ok((proofs, root)) => {
            if proofs.len() != 1 {
                return Err(InputError::UnexpectedProofCount(proofs.len()));
            }
            let (leaf_index, siblings) = proofs
                .into_iter()
                .next()
                .flatten()
                .ok_or(InputError::MissingMembershipProof)?;
            (
                MembershipProof {
                    leaf_index,
                    siblings,
                },
                root,
            )
        }
        Err(current_error) => {
            let legacy: SequencerMembershipProof = client
                .request("getProofForCommitment", rpc_params![*commitment.as_bytes()])
                .await
                .map_err(|legacy_error| {
                    InputError::RpcRequest(format!(
                        "getProofsAndRoot: {current_error}; \
                         getProofForCommitment: {legacy_error}"
                    ))
                })?;
            let (leaf_index, siblings) = legacy.ok_or(InputError::MissingMembershipProof)?;
            let membership_proof = MembershipProof {
                leaf_index,
                siblings,
            };
            let commitment_root = membership_proof.compute_root(&commitment);
            (membership_proof, commitment_root)
        }
    };

    let input = ProverInputFile {
        account: account.clone(),
        membership_proof,
        commitment_root,
    };
    input.validate_membership()?;
    Ok(input)
}

pub fn capture_wallet_snapshot(
    wallet_binary: impl Into<PathBuf>,
    account_mention: &str,
) -> Result<PrivateAccountSnapshot, InputError> {
    let account_id = private_account_id_from_mention(account_mention)?;
    let output = Command::new(wallet_binary.into())
        .args(["account", "get", "--raw", "--account-id", account_mention])
        .output()?;
    if !output.status.success() {
        return Err(InputError::WalletCommandFailed(output.status));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    account_from_wallet_output(account_id, &stdout)
        .map(|account| PrivateAccountSnapshot { account })
}

pub fn private_account_id_from_mention(value: &str) -> Result<Digest32, InputError> {
    let encoded = value
        .strip_prefix("Private/")
        .ok_or(InputError::InvalidPrivateAccountMention)?;
    let decoded = bs58::decode(encoded)
        .into_vec()
        .map_err(|_| InputError::InvalidAccountBase58)?;
    decoded
        .try_into()
        .map_err(|bytes: Vec<u8>| InputError::InvalidAccountIdLength(bytes.len()))
}

pub fn account_from_wallet_output(
    account_id: Digest32,
    output: &str,
) -> Result<LezAccount, InputError> {
    let json_start = output.find('{').ok_or(InputError::MissingWalletJson)?;
    let wallet_account: WalletAccountJson = serde_json::from_str(&output[json_start..])?;
    wallet_account.into_lez_account(account_id)
}

pub fn read_private_snapshot(path: impl AsRef<Path>) -> Result<PrivateAccountSnapshot, InputError> {
    read_json(path)
}

pub fn read_prover_input(path: impl AsRef<Path>) -> Result<ProverInputFile, InputError> {
    let input: ProverInputFile = read_json(path)?;
    input.validate_membership()?;
    Ok(input)
}

pub fn read_proof(path: impl AsRef<Path>) -> Result<Risc0AttestationProof, InputError> {
    read_json(path)
}

pub fn write_private_snapshot(
    path: impl AsRef<Path>,
    snapshot: &PrivateAccountSnapshot,
) -> Result<(), InputError> {
    write_private_json(path.as_ref(), snapshot)
}

pub fn write_prover_input(
    path: impl AsRef<Path>,
    input: &ProverInputFile,
) -> Result<(), InputError> {
    input.validate_membership()?;
    write_private_json(path.as_ref(), input)
}

pub fn write_proof(
    path: impl AsRef<Path>,
    proof: &Risc0AttestationProof,
) -> Result<(), InputError> {
    write_json(path.as_ref(), proof)
}

fn read_json<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T, InputError> {
    Ok(serde_json::from_slice(&fs::read(path)?)?)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), InputError> {
    create_parent(path)?;
    let mut file = File::create(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn write_private_json<T: Serialize>(path: &Path, value: &T) -> Result<(), InputError> {
    create_parent(path)?;
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    Ok(())
}

fn create_parent(path: &Path) -> Result<(), InputError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct WalletAccountJson {
    balance: u128,
    program_owner: String,
    data: String,
    nonce: u128,
}

impl WalletAccountJson {
    fn into_lez_account(self, account_id: Digest32) -> Result<LezAccount, InputError> {
        let owner_bytes = bs58::decode(&self.program_owner)
            .into_vec()
            .map_err(|_| InputError::InvalidProgramOwnerBase58)?;
        if owner_bytes.len() != 32 {
            return Err(InputError::InvalidProgramOwnerLength(owner_bytes.len()));
        }
        let mut program_owner = [0_u32; 8];
        for (index, chunk) in owner_bytes.chunks_exact(4).enumerate() {
            program_owner[index] =
                u32::from_le_bytes(chunk.try_into().expect("chunk has four bytes"));
        }
        let data = hex::decode(&self.data).map_err(|_| InputError::InvalidAccountDataHex)?;

        Ok(LezAccount {
            account_id,
            program_owner,
            balance: self.balance,
            nonce: self.nonce,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::GateContext;
    use lez_compat::FungibleTokenHolding;
    use tokenstudio_config::{TokenConfig, GATE_CONFIG_VERSION, TOKEN_CONFIG_VERSION};

    fn valid_request() -> ProofRequest {
        let definition_id = [0x44; 32];
        let account = LezAccount {
            account_id: [0x22; 32],
            program_owner: [7; 8],
            balance: 5,
            nonce: 9,
            data: FungibleTokenHolding {
                definition_id,
                balance: 250,
            }
            .encode()
            .to_vec(),
        };
        let membership_proof = MembershipProof {
            leaf_index: 0,
            siblings: Vec::new(),
        };
        let commitment_root = membership_proof.compute_root(&account.commitment());
        let token = TokenConfig {
            schema_version: TOKEN_CONFIG_VERSION,
            name: "Founders Token".to_owned(),
            symbol: "FNDR".to_owned(),
            decimals: 0,
            definition_account_id: definition_id,
            token_program_owner: [7; 8],
            definition_account: "Private/founders-definition".to_owned(),
            supply_account: "Private/founders-supply".to_owned(),
            issuer_account: "Private/founders-issuer".to_owned(),
            total_supply: 1_000_000,
        };

        ProofRequest {
            gate: GateConfig {
                schema_version: GATE_CONFIG_VERSION,
                token,
                context: GateContext {
                    application_id: "tokenstudio".to_owned(),
                    gate_id: "founders-chat".to_owned(),
                    token_program_owner: [7; 8],
                    token_definition_id: definition_id,
                    threshold: 100,
                    verifier_id: "logos-chat:founders".to_owned(),
                    expires_at_unix_ms: Some(1_900_000_000_000),
                },
            },
            input: ProverInputFile {
                account,
                membership_proof,
                commitment_root,
            },
            presenter_public_key: [0x55; 32],
            issued_at_unix_ms: 1_800_000_000_000,
        }
    }

    #[test]
    fn proof_request_rejects_tampered_root_before_proving() {
        let mut request = valid_request();
        request.input.commitment_root[0] ^= 1;

        assert!(matches!(
            request.witness(),
            Err(InputError::MembershipRootMismatch)
        ));
    }

    #[test]
    fn wallet_json_parser_matches_lez_human_readable_account() {
        let owner_bytes: Vec<u8> = (0_u8..32).collect();
        let output = format!(
            "Label: founders\n{{\n  \"balance\": 5,\n  \"program_owner\": \"{}\",\n  \"data\": \"0001ff\",\n  \"nonce\": 9\n}}\n",
            bs58::encode(&owner_bytes).into_string()
        );
        let account = account_from_wallet_output([0x22; 32], &output).unwrap();

        assert_eq!(account.account_id, [0x22; 32]);
        assert_eq!(account.balance, 5);
        assert_eq!(account.nonce, 9);
        assert_eq!(account.data, [0, 1, 255]);
        assert_eq!(account.program_owner[0], 0x0302_0100);
        assert_eq!(account.program_owner[7], 0x1f1e_1d1c);
    }

    #[test]
    fn private_account_mention_decodes_base58() {
        let encoded = bs58::encode([0x22; 32]).into_string();
        assert_eq!(
            private_account_id_from_mention(&format!("Private/{encoded}")).unwrap(),
            [0x22; 32]
        );
        assert!(matches!(
            private_account_id_from_mention(&format!("Public/{encoded}")),
            Err(InputError::InvalidPrivateAccountMention)
        ));
    }

    #[tokio::test]
    #[ignore = "requires a standalone LEZ v0.2.0 sequencer on port 3040"]
    async fn pinned_v0_2_sequencer_membership_fallback() {
        let sequencer_url = std::env::var("LEZ_SEQUENCER_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:3040".to_owned());
        let input = fetch_sequencer_input(&LezAccount::default(), &sequencer_url)
            .await
            .unwrap();

        input.validate_membership().unwrap();
        assert_eq!(
            fetch_sequencer_commitment_root(&sequencer_url)
                .await
                .unwrap(),
            input.commitment_root
        );
    }
}

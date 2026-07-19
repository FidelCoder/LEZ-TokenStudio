use std::env;

use attestation_circuit::{evaluate, BalanceAttestationWitness, CircuitError};
use attestation_types::AttestationJournal;
use balance_attestation_methods::{BALANCE_ATTESTATION_ELF, BALANCE_ATTESTATION_ID};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use risc0_zkvm::{default_prover, ExecutorEnv, ProverOpts, Receipt};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

mod input;
pub use input::{
    account_from_wallet_output, capture_wallet_snapshot, fetch_sequencer_input,
    private_account_id_from_mention, prove_live, read_private_snapshot, read_proof,
    read_prover_input, write_private_snapshot, write_proof, write_prover_input, InputError,
    PrivateAccountSnapshot, ProofRequest, ProverInputFile,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Risc0AttestationProof {
    pub journal: AttestationJournal,
    #[serde(with = "receipt_base64")]
    pub receipt: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DevModeStatus {
    Disabled,
    Enabled,
}

#[derive(Debug, Error)]
pub enum ProverError {
    #[error("private witness does not satisfy the attestation statement: {0}")]
    InvalidWitness(#[from] CircuitError),
    #[error(transparent)]
    Input(#[from] InputError),
    #[error("RISC0_DEV_MODE must be 0 or unset for a real ProofGate proof")]
    DevModeEnabled,
    #[error("RISC0_DEV_MODE has unsupported value '{0}'")]
    InvalidDevMode(String),
    #[error("failed to prepare Risc0 executor input: {0}")]
    ExecutorInput(String),
    #[error("Risc0 proof generation failed: {0}")]
    Proving(String),
    #[error("Risc0 receipt verification failed: {0}")]
    ReceiptVerification(String),
    #[error("Risc0 journal decoding failed: {0}")]
    JournalDecode(String),
    #[error("Risc0 receipt encoding failed: {0}")]
    ReceiptEncode(String),
    #[error("Risc0 receipt decoding failed: {0}")]
    ReceiptDecode(String),
    #[error("guest journal did not match the host-evaluated statement")]
    JournalMismatch,
}

pub fn dev_mode_status() -> Result<DevModeStatus, ProverError> {
    match env::var("RISC0_DEV_MODE") {
        Err(env::VarError::NotPresent) => parse_dev_mode(None),
        Err(env::VarError::NotUnicode(_)) => {
            Err(ProverError::InvalidDevMode("non-Unicode value".to_owned()))
        }
        Ok(value) => parse_dev_mode(Some(&value)),
    }
}

fn parse_dev_mode(value: Option<&str>) -> Result<DevModeStatus, ProverError> {
    match value {
        None => Ok(DevModeStatus::Disabled),
        Some(value) => match value.trim().to_ascii_lowercase().as_str() {
            "" | "0" | "false" | "off" => Ok(DevModeStatus::Disabled),
            "1" | "true" | "on" => Ok(DevModeStatus::Enabled),
            _ => Err(ProverError::InvalidDevMode(value.to_owned())),
        },
    }
}

pub fn ensure_real_proving_mode() -> Result<(), ProverError> {
    match dev_mode_status()? {
        DevModeStatus::Disabled => Ok(()),
        DevModeStatus::Enabled => Err(ProverError::DevModeEnabled),
    }
}

pub fn prove(witness: &BalanceAttestationWitness) -> Result<Risc0AttestationProof, ProverError> {
    ensure_real_proving_mode()?;
    let expected_journal = evaluate(witness)?;

    let env = ExecutorEnv::builder()
        .write(witness)
        .map_err(|error| ProverError::ExecutorInput(error.to_string()))?
        .build()
        .map_err(|error| ProverError::ExecutorInput(error.to_string()))?;
    let prove_info = default_prover()
        .prove_with_opts(env, BALANCE_ATTESTATION_ELF, &ProverOpts::succinct())
        .map_err(|error| ProverError::Proving(error.to_string()))?;
    prove_info
        .receipt
        .verify(BALANCE_ATTESTATION_ID)
        .map_err(|error| ProverError::ReceiptVerification(error.to_string()))?;

    let journal = prove_info
        .receipt
        .journal
        .decode::<AttestationJournal>()
        .map_err(|error| ProverError::JournalDecode(error.to_string()))?;
    if journal != expected_journal {
        return Err(ProverError::JournalMismatch);
    }

    let receipt = bincode::serialize(&prove_info.receipt)
        .map_err(|error| ProverError::ReceiptEncode(error.to_string()))?;

    Ok(Risc0AttestationProof { journal, receipt })
}

pub fn prove_request(request: &ProofRequest) -> Result<Risc0AttestationProof, ProverError> {
    prove(&request.witness()?)
}

pub fn verify_receipt(proof: &Risc0AttestationProof) -> Result<AttestationJournal, ProverError> {
    let receipt = decode_receipt(&proof.receipt)?;
    receipt
        .verify(BALANCE_ATTESTATION_ID)
        .map_err(|error| ProverError::ReceiptVerification(error.to_string()))?;
    let journal = receipt
        .journal
        .decode::<AttestationJournal>()
        .map_err(|error| ProverError::JournalDecode(error.to_string()))?;
    if journal != proof.journal {
        return Err(ProverError::JournalMismatch);
    }
    Ok(journal)
}

pub fn decode_receipt(bytes: &[u8]) -> Result<Receipt, ProverError> {
    bincode::deserialize(bytes).map_err(|error| ProverError::ReceiptDecode(error.to_string()))
}

mod receipt_base64 {
    use super::*;

    pub fn serialize<S>(value: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&BASE64.encode(value))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        BASE64.decode(value).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::GateContext;
    use lez_compat::{FungibleTokenHolding, LezAccount, MembershipProof};
    use risc0_zkvm::default_executor;

    fn valid_witness() -> BalanceAttestationWitness {
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
        let commitment = account.commitment();
        let membership_proof = MembershipProof {
            leaf_index: 0,
            siblings: Vec::new(),
        };
        let commitment_root = membership_proof.compute_root(&commitment);

        BalanceAttestationWitness {
            account,
            membership_proof,
            commitment_root,
            gate_context: GateContext {
                application_id: "tokenstudio".to_owned(),
                gate_id: "founders-chat".to_owned(),
                token_program_owner: [7; 8],
                token_definition_id: definition_id,
                threshold: 100,
                verifier_id: "logos-chat:founders".to_owned(),
                expires_at_unix_ms: Some(1_900_000_000_000),
            },
            presenter_public_key: [0x55; 32],
            issued_at_unix_ms: 1_800_000_000_000,
        }
    }

    #[test]
    fn guest_executes_same_statement_as_host() {
        let witness = valid_witness();
        let expected = evaluate(&witness).unwrap();
        let env = ExecutorEnv::builder()
            .write(&witness)
            .unwrap()
            .build()
            .unwrap();
        let session = default_executor()
            .execute(env, BALANCE_ATTESTATION_ELF)
            .unwrap();
        let actual = session.journal.decode::<AttestationJournal>().unwrap();

        assert_eq!(actual, expected);
    }

    #[test]
    fn guest_rejects_insufficient_balance() {
        let mut witness = valid_witness();
        witness.gate_context.threshold = 251;
        let env = ExecutorEnv::builder()
            .write(&witness)
            .unwrap()
            .build()
            .unwrap();

        assert!(default_executor()
            .execute(env, BALANCE_ATTESTATION_ELF)
            .is_err());
    }

    #[test]
    fn proof_json_uses_base64_receipt() {
        let proof = Risc0AttestationProof {
            journal: evaluate(&valid_witness()).unwrap(),
            receipt: vec![0, 1, 2, 253, 254, 255],
        };
        let json = serde_json::to_string(&proof).unwrap();

        assert!(json.contains("\"receipt\":\"AAEC/f7/\""));
        assert_eq!(
            serde_json::from_str::<Risc0AttestationProof>(&json).unwrap(),
            proof
        );
    }

    #[test]
    fn dev_mode_parser_is_strict() {
        assert_eq!(parse_dev_mode(None).unwrap(), DevModeStatus::Disabled);
        assert_eq!(parse_dev_mode(Some("0")).unwrap(), DevModeStatus::Disabled);
        assert_eq!(
            parse_dev_mode(Some("true")).unwrap(),
            DevModeStatus::Enabled
        );
        assert!(parse_dev_mode(Some("sometimes")).is_err());
    }

    #[test]
    #[ignore = "generates a real succinct proof and requires RISC0_DEV_MODE=0"]
    fn real_succinct_proof_round_trip() {
        assert_eq!(env::var("RISC0_DEV_MODE").unwrap(), "0");
        let witness = valid_witness();
        let proof = prove(&witness).unwrap();
        let journal = verify_receipt(&proof).unwrap();

        assert_eq!(journal, evaluate(&witness).unwrap());
        assert!(proof.receipt.len() > 1_000);
    }
}

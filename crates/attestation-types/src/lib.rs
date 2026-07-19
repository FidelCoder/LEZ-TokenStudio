use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const GATE_CONTEXT_DOMAIN: &[u8] = b"LEZ-TokenStudio/GateContext/v1";
pub const ATTESTATION_JOURNAL_VERSION: u16 = 1;

pub type Digest32 = [u8; 32];
pub type ProgramOwner = [u32; 8];
pub type PublicKey32 = [u8; 32];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateContext {
    pub application_id: String,
    pub gate_id: String,
    pub token_program_owner: ProgramOwner,
    pub threshold: u128,
    pub verifier_id: String,
    pub expires_at_unix_ms: Option<u64>,
}

impl GateContext {
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(GATE_CONTEXT_DOMAIN);
        write_string(&mut bytes, &self.application_id);
        write_string(&mut bytes, &self.gate_id);
        for word in self.token_program_owner {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&self.threshold.to_le_bytes());
        write_string(&mut bytes, &self.verifier_id);
        match self.expires_at_unix_ms {
            Some(expires_at) => {
                bytes.push(1);
                bytes.extend_from_slice(&expires_at.to_le_bytes());
            }
            None => bytes.push(0),
        }
        bytes
    }

    #[must_use]
    pub fn context_hash(&self) -> Digest32 {
        sha256(&self.canonical_bytes())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationJournal {
    pub version: u16,
    pub circuit_id: Digest32,
    pub context_hash: Digest32,
    pub token_program_owner: ProgramOwner,
    pub threshold: u128,
    pub commitment_root: Digest32,
    pub presenter_public_key: PublicKey32,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: Option<u64>,
}

impl AttestationJournal {
    #[must_use]
    pub fn new(
        circuit_id: Digest32,
        gate_context: &GateContext,
        commitment_root: Digest32,
        presenter_public_key: PublicKey32,
        issued_at_unix_ms: u64,
    ) -> Self {
        Self {
            version: ATTESTATION_JOURNAL_VERSION,
            circuit_id,
            context_hash: gate_context.context_hash(),
            token_program_owner: gate_context.token_program_owner,
            threshold: gate_context.threshold,
            commitment_root,
            presenter_public_key,
            issued_at_unix_ms,
            expires_at_unix_ms: gate_context.expires_at_unix_ms,
        }
    }

    #[must_use]
    pub fn is_expired(&self, now_unix_ms: u64) -> bool {
        self.expires_at_unix_ms
            .is_some_and(|expires_at| now_unix_ms > expires_at)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProofTransport {
    OnChain,
    LogosMessaging,
    Local,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttestationEnvelope {
    pub journal: AttestationJournal,
    pub receipt: Vec<u8>,
    pub challenge: Vec<u8>,
    pub presenter_signature: Vec<u8>,
    pub transport: ProofTransport,
}

impl AttestationEnvelope {
    #[must_use]
    pub fn context_hash(&self) -> Digest32 {
        self.journal.context_hash
    }
}

#[must_use]
pub fn sha256(bytes: &[u8]) -> Digest32 {
    Sha256::digest(bytes).into()
}

#[must_use]
pub fn digest_to_hex(digest: &Digest32) -> String {
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(hex_nibble(byte >> 4));
        output.push(hex_nibble(byte & 0x0f));
    }
    output
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    let value_bytes = value.as_bytes();
    let len = u32::try_from(value_bytes.len()).expect("gate context string is too long");
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value_bytes);
}

fn hex_nibble(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + value - 10),
        _ => unreachable!("hex nibble must be in 0..=15"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_context() -> GateContext {
        GateContext {
            application_id: "tokenstudio".to_owned(),
            gate_id: "founders-chat".to_owned(),
            token_program_owner: [
                0x0302_0100,
                0x0706_0504,
                0x0b0a_0908,
                0x0f0e_0d0c,
                0x1312_1110,
                0x1716_1514,
                0x1b1a_1918,
                0x1f1e_1d1c,
            ],
            threshold: 100,
            verifier_id: "logos-chat:founders".to_owned(),
            expires_at_unix_ms: Some(1_800_000_000_000),
        }
    }

    #[test]
    fn gate_context_hash_is_stable() {
        let hash = sample_context().context_hash();
        assert_eq!(digest_to_hex(&hash).len(), 64);
        assert_eq!(hash, sample_context().context_hash());
    }

    #[test]
    fn journal_uses_gate_context_values() {
        let context = sample_context();
        let journal = AttestationJournal::new([1; 32], &context, [2; 32], [3; 32], 42);

        assert_eq!(journal.version, ATTESTATION_JOURNAL_VERSION);
        assert_eq!(journal.context_hash, context.context_hash());
        assert_eq!(journal.token_program_owner, context.token_program_owner);
        assert_eq!(journal.threshold, context.threshold);
        assert_eq!(journal.expires_at_unix_ms, context.expires_at_unix_ms);
        assert!(!journal.is_expired(1_700_000_000_000));
        assert!(journal.is_expired(1_900_000_000_000));
    }
}

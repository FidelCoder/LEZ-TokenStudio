use std::fmt;

use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};

pub const GATE_CONTEXT_DOMAIN: &[u8] = b"LEZ-TokenStudio/GateContext/v2";
pub const ATTESTATION_JOURNAL_VERSION: u16 = 2;

pub type Digest32 = [u8; 32];
pub type ProgramOwner = [u32; 8];
pub type PublicKey32 = [u8; 32];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseHexError {
    message: String,
}

impl ParseHexError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ParseHexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ParseHexError {}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateContext {
    pub application_id: String,
    pub gate_id: String,
    #[serde(with = "serde_program_owner_hex")]
    pub token_program_owner: ProgramOwner,
    #[serde(with = "serde_digest_hex")]
    pub token_definition_id: Digest32,
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
        bytes.extend_from_slice(&self.token_definition_id);
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
    #[serde(with = "serde_digest_hex")]
    pub context_hash: Digest32,
    #[serde(with = "serde_program_owner_hex")]
    pub token_program_owner: ProgramOwner,
    #[serde(with = "serde_digest_hex")]
    pub token_definition_id: Digest32,
    pub threshold: u128,
    #[serde(with = "serde_digest_hex")]
    pub commitment_root: Digest32,
    #[serde(with = "serde_digest_hex")]
    pub presenter_public_key: PublicKey32,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: Option<u64>,
}

impl AttestationJournal {
    #[must_use]
    pub fn new(
        gate_context: &GateContext,
        commitment_root: Digest32,
        presenter_public_key: PublicKey32,
        issued_at_unix_ms: u64,
    ) -> Self {
        Self {
            version: ATTESTATION_JOURNAL_VERSION,
            context_hash: gate_context.context_hash(),
            token_program_owner: gate_context.token_program_owner,
            token_definition_id: gate_context.token_definition_id,
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

pub fn digest_from_hex(value: &str) -> Result<Digest32, ParseHexError> {
    let value = strip_hex_prefix(value);
    if value.len() != 64 {
        return Err(ParseHexError::new(
            "digest must be exactly 32 bytes / 64 hexadecimal characters",
        ));
    }

    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        let offset = index * 2;
        *byte = parse_hex_byte(&value[offset..offset + 2])?;
    }
    Ok(bytes)
}

#[must_use]
pub fn program_owner_to_hex(owner: &ProgramOwner) -> String {
    let mut bytes = [0_u8; 32];
    for (index, word) in owner.iter().enumerate() {
        let start = index * 4;
        bytes[start..start + 4].copy_from_slice(&word.to_le_bytes());
    }
    digest_to_hex(&bytes)
}

pub fn program_owner_from_hex(value: &str) -> Result<ProgramOwner, ParseHexError> {
    let bytes = digest_from_hex(value)?;
    let mut owner = [0_u32; 8];
    for (index, chunk) in bytes.chunks_exact(4).enumerate() {
        owner[index] = u32::from_le_bytes(chunk.try_into().expect("chunk has four bytes"));
    }
    Ok(owner)
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    let value_bytes = value.as_bytes();
    let len = u32::try_from(value_bytes.len()).expect("gate context string is too long");
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value_bytes);
}

fn strip_hex_prefix(value: &str) -> &str {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value)
}

fn parse_hex_byte(value: &str) -> Result<u8, ParseHexError> {
    u8::from_str_radix(value, 16)
        .map_err(|_| ParseHexError::new(format!("invalid hexadecimal byte '{value}'")))
}

fn hex_nibble(value: u8) -> char {
    match value {
        0..=9 => char::from(b'0' + value),
        10..=15 => char::from(b'a' + value - 10),
        _ => unreachable!("hex nibble must be in 0..=15"),
    }
}

pub mod serde_digest_hex {
    use super::*;

    pub fn serialize<S>(value: &Digest32, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&digest_to_hex(value))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Digest32, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        digest_from_hex(&value).map_err(D::Error::custom)
    }
}

pub mod serde_program_owner_hex {
    use super::*;

    pub fn serialize<S>(value: &ProgramOwner, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&program_owner_to_hex(value))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<ProgramOwner, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        program_owner_from_hex(&value).map_err(D::Error::custom)
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
            token_definition_id: [0x42; 32],
            threshold: 100,
            verifier_id: "logos-chat:founders".to_owned(),
            expires_at_unix_ms: Some(1_800_000_000_000),
        }
    }

    #[test]
    fn gate_context_hash_is_stable_and_asset_specific() {
        let context = sample_context();
        let hash = context.context_hash();
        assert_eq!(digest_to_hex(&hash).len(), 64);
        assert_eq!(hash, sample_context().context_hash());

        let mut another_asset = context;
        another_asset.token_definition_id[0] ^= 1;
        assert_ne!(hash, another_asset.context_hash());
    }

    #[test]
    fn journal_uses_gate_context_values() {
        let context = sample_context();
        let journal = AttestationJournal::new(&context, [2; 32], [3; 32], 42);

        assert_eq!(journal.version, ATTESTATION_JOURNAL_VERSION);
        assert_eq!(journal.context_hash, context.context_hash());
        assert_eq!(journal.token_program_owner, context.token_program_owner);
        assert_eq!(journal.token_definition_id, context.token_definition_id);
        assert_eq!(journal.threshold, context.threshold);
        assert_eq!(journal.expires_at_unix_ms, context.expires_at_unix_ms);
        assert!(!journal.is_expired(1_700_000_000_000));
        assert!(journal.is_expired(1_900_000_000_000));
    }

    #[test]
    fn hex_conversions_preserve_le_program_words() {
        let expected = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
        let owner = program_owner_from_hex(expected).unwrap();

        assert_eq!(
            owner,
            [
                0x0302_0100,
                0x0706_0504,
                0x0b0a_0908,
                0x0f0e_0d0c,
                0x1312_1110,
                0x1716_1514,
                0x1b1a_1918,
                0x1f1e_1d1c,
            ]
        );
        assert_eq!(program_owner_to_hex(&owner), expected);
        assert_eq!(digest_from_hex(&format!("0x{expected}")).unwrap(), {
            let mut bytes = [0_u8; 32];
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = u8::try_from(index).unwrap();
            }
            bytes
        });
    }

    #[test]
    fn gate_context_json_uses_compact_hex_ids() {
        let json = serde_json::to_string(&sample_context()).unwrap();

        assert!(json.contains(
            "\"token_program_owner\":\"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f\""
        ));
        assert!(json.contains(&format!("\"token_definition_id\":\"{}\"", "42".repeat(32))));
        assert_eq!(
            serde_json::from_str::<GateContext>(&json).unwrap(),
            sample_context()
        );
    }
}

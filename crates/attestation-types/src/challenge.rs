use serde::{Deserialize, Serialize};

use crate::{sha256, Digest32};

pub const VERIFICATION_CHALLENGE_VERSION: u16 = 2;
pub const VERIFICATION_CHALLENGE_DOMAIN: &[u8] = b"LEZ-TokenStudio/VerificationChallenge/v2";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationChallenge {
    pub version: u16,
    #[serde(with = "crate::serde_digest_hex")]
    pub gate_context_hash: Digest32,
    #[serde(with = "crate::serde_digest_hex")]
    pub expected_commitment_root: Digest32,
    pub verifier_id: String,
    #[serde(with = "crate::serde_digest_hex")]
    pub nonce: Digest32,
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: u64,
}

impl VerificationChallenge {
    #[must_use]
    pub fn new(
        gate_context_hash: Digest32,
        expected_commitment_root: Digest32,
        verifier_id: String,
        nonce: Digest32,
        issued_at_unix_ms: u64,
        expires_at_unix_ms: u64,
    ) -> Self {
        Self {
            version: VERIFICATION_CHALLENGE_VERSION,
            gate_context_hash,
            expected_commitment_root,
            verifier_id,
            nonce,
            issued_at_unix_ms,
            expires_at_unix_ms,
        }
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(VERIFICATION_CHALLENGE_DOMAIN);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        bytes.extend_from_slice(&self.gate_context_hash);
        bytes.extend_from_slice(&self.expected_commitment_root);
        write_string(&mut bytes, &self.verifier_id);
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.issued_at_unix_ms.to_le_bytes());
        bytes.extend_from_slice(&self.expires_at_unix_ms.to_le_bytes());
        bytes
    }

    #[must_use]
    pub fn digest(&self) -> Digest32 {
        sha256(&self.canonical_bytes())
    }

    #[must_use]
    pub fn is_expired(&self, now_unix_ms: u64) -> bool {
        now_unix_ms > self.expires_at_unix_ms
    }

    #[must_use]
    pub fn has_valid_window(&self) -> bool {
        self.issued_at_unix_ms <= self.expires_at_unix_ms
    }
}

fn write_string(bytes: &mut Vec<u8>, value: &str) {
    let value_bytes = value.as_bytes();
    let len = u32::try_from(value_bytes.len()).expect("verifier id is too long");
    bytes.extend_from_slice(&len.to_le_bytes());
    bytes.extend_from_slice(value_bytes);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn challenge_digest_binds_every_field() {
        let challenge = VerificationChallenge::new(
            [1; 32],
            [3; 32],
            "logos-chat:founders".to_owned(),
            [2; 32],
            10,
            20,
        );
        let expected = challenge.digest();

        let mut changed = challenge.clone();
        changed.nonce[0] ^= 1;
        assert_ne!(changed.digest(), expected);

        changed = challenge.clone();
        changed.verifier_id.push('2');
        assert_ne!(changed.digest(), expected);

        changed = challenge.clone();
        changed.gate_context_hash[0] ^= 1;
        assert_ne!(changed.digest(), expected);
        changed = challenge.clone();
        changed.expected_commitment_root[0] ^= 1;
        assert_ne!(changed.digest(), expected);

        changed = challenge;
        changed.expires_at_unix_ms += 1;
        assert_ne!(changed.digest(), expected);
    }

    #[test]
    fn challenge_window_is_explicit() {
        let challenge =
            VerificationChallenge::new([1; 32], [3; 32], "verifier".to_owned(), [2; 32], 10, 20);

        assert!(challenge.has_valid_window());
        assert!(!challenge.is_expired(20));
        assert!(challenge.is_expired(21));
    }
}

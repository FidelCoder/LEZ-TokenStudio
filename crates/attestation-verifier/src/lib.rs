use std::collections::HashSet;

use attestation_prover::{verify_receipt, Risc0AttestationProof};
use attestation_types::{
    sha256, AttestationEnvelope, Digest32, ProofTransport, VerificationChallenge,
    ATTESTATION_JOURNAL_VERSION, VERIFICATION_CHALLENGE_VERSION,
};
use ed25519_dalek::{Signature, Signer as _, SigningKey, Verifier as _, VerifyingKey};
use rand::{rngs::OsRng, RngCore as _};
use thiserror::Error;
use tokenstudio_config::GateConfig;

pub const PRESENTATION_BINDING_DOMAIN: &[u8] = b"LEZ-TokenStudio/PresentationBinding/v1";
pub const MAX_CHALLENGE_TTL_MS: u64 = 5 * 60 * 1_000;
pub const MAX_CLOCK_SKEW_MS: u64 = 30_000;
pub const MAX_PROOF_AGE_MS: u64 = 10 * 60 * 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum VerificationErrorCode {
    InvalidGate = 1000,
    InvalidProof = 1001,
    WrongContext = 1002,
    WrongToken = 1003,
    WrongThreshold = 1004,
    ExpiredProof = 1005,
    InvalidChallenge = 1006,
    ExpiredChallenge = 1007,
    WrongVerifier = 1008,
    BadPresenter = 1009,
    ReplayedChallenge = 1010,
    MalformedEnvelope = 1011,
    InvalidProofTime = 1012,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VerificationError {
    #[error("gate configuration is invalid")]
    InvalidGate,
    #[error("Risc0 receipt is invalid")]
    InvalidProof,
    #[error("proof or challenge is bound to a different gate context")]
    WrongContext,
    #[error("proof is for a different token")]
    WrongToken,
    #[error("proof threshold does not equal the gate threshold")]
    WrongThreshold,
    #[error("proof has expired")]
    ExpiredProof,
    #[error("challenge is malformed or has an invalid time window")]
    InvalidChallenge,
    #[error("challenge has expired")]
    ExpiredChallenge,
    #[error("challenge is for a different verifier")]
    WrongVerifier,
    #[error("presenter key or signature is invalid")]
    BadPresenter,
    #[error("challenge has already been consumed")]
    ReplayedChallenge,
    #[error("attestation envelope is malformed")]
    MalformedEnvelope,
    #[error("proof issuance time is stale, predates the challenge, or is in the future")]
    InvalidProofTime,
}

impl VerificationError {
    #[must_use]
    pub const fn code(&self) -> VerificationErrorCode {
        match self {
            Self::InvalidGate => VerificationErrorCode::InvalidGate,
            Self::InvalidProof => VerificationErrorCode::InvalidProof,
            Self::WrongContext => VerificationErrorCode::WrongContext,
            Self::WrongToken => VerificationErrorCode::WrongToken,
            Self::WrongThreshold => VerificationErrorCode::WrongThreshold,
            Self::ExpiredProof => VerificationErrorCode::ExpiredProof,
            Self::InvalidChallenge => VerificationErrorCode::InvalidChallenge,
            Self::ExpiredChallenge => VerificationErrorCode::ExpiredChallenge,
            Self::WrongVerifier => VerificationErrorCode::WrongVerifier,
            Self::BadPresenter => VerificationErrorCode::BadPresenter,
            Self::ReplayedChallenge => VerificationErrorCode::ReplayedChallenge,
            Self::MalformedEnvelope => VerificationErrorCode::MalformedEnvelope,
            Self::InvalidProofTime => VerificationErrorCode::InvalidProofTime,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VerifiedAttestation {
    pub context_hash: Digest32,
    pub commitment_root: Digest32,
    pub presenter_public_key: Digest32,
    pub issued_at_unix_ms: u64,
}

pub struct VerifyRequest<'a> {
    pub envelope: &'a AttestationEnvelope,
    pub expected_gate: &'a GateConfig,
    pub expected_verifier_id: &'a str,
    pub now_unix_ms: u64,
}

#[derive(Debug, Default)]
pub struct InMemoryReplayCache {
    consumed: HashSet<Digest32>,
}

impl InMemoryReplayCache {
    #[must_use]
    pub fn from_consumed(consumed: impl IntoIterator<Item = Digest32>) -> Self {
        Self {
            consumed: consumed.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.consumed.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.consumed.is_empty()
    }

    pub fn consumed(&self) -> impl Iterator<Item = &Digest32> {
        self.consumed.iter()
    }

    fn consume(&mut self, challenge_digest: Digest32) -> bool {
        self.consumed.insert(challenge_digest)
    }
}

pub fn create_challenge(
    gate: &GateConfig,
    verifier_id: &str,
    now_unix_ms: u64,
    ttl_ms: u64,
) -> Result<VerificationChallenge, VerificationError> {
    gate.validate()
        .map_err(|_| VerificationError::InvalidGate)?;
    if verifier_id != gate.context.verifier_id {
        return Err(VerificationError::WrongVerifier);
    }
    if ttl_ms == 0 || ttl_ms > MAX_CHALLENGE_TTL_MS {
        return Err(VerificationError::InvalidChallenge);
    }
    let expires_at_unix_ms = now_unix_ms
        .checked_add(ttl_ms)
        .ok_or(VerificationError::InvalidChallenge)?;
    let mut nonce = [0_u8; 32];
    OsRng.fill_bytes(&mut nonce);

    Ok(VerificationChallenge::new(
        gate.context_hash(),
        verifier_id.to_owned(),
        nonce,
        now_unix_ms,
        expires_at_unix_ms,
    ))
}

pub fn present(
    proof: &Risc0AttestationProof,
    challenge: VerificationChallenge,
    signing_key: &SigningKey,
    transport: ProofTransport,
) -> Result<AttestationEnvelope, VerificationError> {
    let presenter_public_key = signing_key.verifying_key().to_bytes();
    if proof.journal.presenter_public_key != presenter_public_key {
        return Err(VerificationError::BadPresenter);
    }
    if proof.journal.context_hash != challenge.gate_context_hash {
        return Err(VerificationError::WrongContext);
    }

    let signature = signing_key.sign(&presentation_message(&challenge, &proof.receipt));
    Ok(AttestationEnvelope {
        journal: proof.journal.clone(),
        receipt: proof.receipt.clone(),
        challenge,
        presenter_signature: signature.to_bytes().to_vec(),
        transport,
    })
}

pub fn verify(
    request: VerifyRequest<'_>,
    replay_cache: &mut InMemoryReplayCache,
) -> Result<VerifiedAttestation, VerificationError> {
    validate_claims_and_signature(&request)?;

    let proof = Risc0AttestationProof {
        journal: request.envelope.journal.clone(),
        receipt: request.envelope.receipt.clone(),
    };
    let verified_journal = verify_receipt(&proof).map_err(|_| VerificationError::InvalidProof)?;
    if verified_journal != request.envelope.journal {
        return Err(VerificationError::InvalidProof);
    }

    if !replay_cache.consume(request.envelope.challenge.digest()) {
        return Err(VerificationError::ReplayedChallenge);
    }

    Ok(VerifiedAttestation {
        context_hash: verified_journal.context_hash,
        commitment_root: verified_journal.commitment_root,
        presenter_public_key: verified_journal.presenter_public_key,
        issued_at_unix_ms: verified_journal.issued_at_unix_ms,
    })
}

fn validate_claims_and_signature(request: &VerifyRequest<'_>) -> Result<(), VerificationError> {
    request
        .expected_gate
        .validate()
        .map_err(|_| VerificationError::InvalidGate)?;
    let envelope = request.envelope;
    let challenge = &envelope.challenge;
    let expected_context_hash = request.expected_gate.context_hash();

    if envelope.journal.version != ATTESTATION_JOURNAL_VERSION {
        return Err(VerificationError::MalformedEnvelope);
    }
    if challenge.version != VERIFICATION_CHALLENGE_VERSION
        || !challenge.has_valid_window()
        || challenge.expires_at_unix_ms - challenge.issued_at_unix_ms > MAX_CHALLENGE_TTL_MS
    {
        return Err(VerificationError::InvalidChallenge);
    }
    if challenge.is_expired(request.now_unix_ms) {
        return Err(VerificationError::ExpiredChallenge);
    }
    if challenge.issued_at_unix_ms > request.now_unix_ms.saturating_add(MAX_CLOCK_SKEW_MS) {
        return Err(VerificationError::InvalidChallenge);
    }
    if challenge.verifier_id != request.expected_verifier_id
        || challenge.verifier_id != request.expected_gate.context.verifier_id
    {
        return Err(VerificationError::WrongVerifier);
    }
    if challenge.gate_context_hash != expected_context_hash
        || envelope.journal.context_hash != expected_context_hash
    {
        return Err(VerificationError::WrongContext);
    }
    if envelope.journal.token_program_owner != request.expected_gate.context.token_program_owner
        || envelope.journal.token_definition_id != request.expected_gate.context.token_definition_id
    {
        return Err(VerificationError::WrongToken);
    }
    if envelope.journal.threshold != request.expected_gate.context.threshold {
        return Err(VerificationError::WrongThreshold);
    }
    if envelope.journal.expires_at_unix_ms != request.expected_gate.context.expires_at_unix_ms {
        return Err(VerificationError::WrongContext);
    }
    if envelope.journal.is_expired(request.now_unix_ms) {
        return Err(VerificationError::ExpiredProof);
    }
    if envelope.journal.issued_at_unix_ms > request.now_unix_ms.saturating_add(MAX_CLOCK_SKEW_MS)
        || request
            .now_unix_ms
            .saturating_sub(envelope.journal.issued_at_unix_ms)
            > MAX_PROOF_AGE_MS
        || envelope
            .journal
            .issued_at_unix_ms
            .saturating_add(MAX_PROOF_AGE_MS)
            < challenge.issued_at_unix_ms
    {
        return Err(VerificationError::InvalidProofTime);
    }

    let verifying_key = VerifyingKey::from_bytes(&envelope.journal.presenter_public_key)
        .map_err(|_| VerificationError::BadPresenter)?;
    let signature = Signature::from_slice(&envelope.presenter_signature)
        .map_err(|_| VerificationError::MalformedEnvelope)?;
    verifying_key
        .verify(
            &presentation_message(challenge, &envelope.receipt),
            &signature,
        )
        .map_err(|_| VerificationError::BadPresenter)
}

#[must_use]
pub fn presentation_message(challenge: &VerificationChallenge, receipt: &[u8]) -> Digest32 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(PRESENTATION_BINDING_DOMAIN);
    bytes.extend_from_slice(&challenge.canonical_bytes());
    bytes.extend_from_slice(&sha256(receipt));
    sha256(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::{AttestationJournal, GateContext};
    use tokenstudio_config::{TokenConfig, GATE_CONFIG_VERSION, TOKEN_CONFIG_VERSION};

    fn gate() -> GateConfig {
        let token = TokenConfig {
            schema_version: TOKEN_CONFIG_VERSION,
            name: "Founders Token".to_owned(),
            symbol: "FNDR".to_owned(),
            decimals: 0,
            definition_account_id: [4; 32],
            token_program_owner: [7; 8],
            definition_account: "Private/founders-definition".to_owned(),
            supply_account: "Private/founders-supply".to_owned(),
            issuer_account: "Private/founders-issuer".to_owned(),
            total_supply: 1_000_000,
        };
        GateConfig {
            schema_version: GATE_CONFIG_VERSION,
            context: GateContext {
                application_id: "tokenstudio".to_owned(),
                gate_id: "founders-chat".to_owned(),
                token_program_owner: token.token_program_owner,
                token_definition_id: token.definition_account_id,
                threshold: 100,
                verifier_id: "logos-chat:founders".to_owned(),
                expires_at_unix_ms: Some(2_000_000),
            },
            token,
        }
    }

    fn proof(signing_key: &SigningKey) -> Risc0AttestationProof {
        let gate = gate();
        Risc0AttestationProof {
            journal: AttestationJournal::new(
                &gate.context,
                [8; 32],
                signing_key.verifying_key().to_bytes(),
                1_000,
            ),
            receipt: vec![9; 128],
        }
    }

    #[test]
    fn presentation_signature_binds_challenge_and_receipt() {
        let gate = gate();
        let signing_key = SigningKey::from_bytes(&[3; 32]);
        let challenge = VerificationChallenge::new(
            gate.context_hash(),
            gate.context.verifier_id.clone(),
            [5; 32],
            1_000,
            1_500,
        );
        let envelope = present(
            &proof(&signing_key),
            challenge.clone(),
            &signing_key,
            ProofTransport::LogosMessaging,
        )
        .unwrap();
        let request = VerifyRequest {
            envelope: &envelope,
            expected_gate: &gate,
            expected_verifier_id: &gate.context.verifier_id,
            now_unix_ms: 1_200,
        };

        assert!(validate_claims_and_signature(&request).is_ok());

        let mut forwarded = envelope.clone();
        forwarded.challenge.nonce[0] ^= 1;
        let request = VerifyRequest {
            envelope: &forwarded,
            expected_gate: &gate,
            expected_verifier_id: &gate.context.verifier_id,
            now_unix_ms: 1_200,
        };
        assert_eq!(
            validate_claims_and_signature(&request).unwrap_err(),
            VerificationError::BadPresenter
        );

        let mut swapped_receipt = envelope;
        swapped_receipt.receipt[0] ^= 1;
        let request = VerifyRequest {
            envelope: &swapped_receipt,
            expected_gate: &gate,
            expected_verifier_id: &gate.context.verifier_id,
            now_unix_ms: 1_200,
        };
        assert_eq!(
            validate_claims_and_signature(&request).unwrap_err(),
            VerificationError::BadPresenter
        );
    }

    #[test]
    fn presenter_key_must_match_key_committed_in_proof() {
        let proof_key = SigningKey::from_bytes(&[3; 32]);
        let other_key = SigningKey::from_bytes(&[4; 32]);
        let gate = gate();
        let challenge = VerificationChallenge::new(
            gate.context_hash(),
            gate.context.verifier_id.clone(),
            [5; 32],
            1_000,
            1_500,
        );

        assert_eq!(
            present(
                &proof(&proof_key),
                challenge,
                &other_key,
                ProofTransport::Local
            )
            .unwrap_err(),
            VerificationError::BadPresenter
        );
    }

    #[test]
    fn challenge_expiry_and_verifier_are_enforced() {
        let gate = gate();
        let signing_key = SigningKey::from_bytes(&[3; 32]);
        let challenge = VerificationChallenge::new(
            gate.context_hash(),
            gate.context.verifier_id.clone(),
            [5; 32],
            1_000,
            1_100,
        );
        let envelope = present(
            &proof(&signing_key),
            challenge,
            &signing_key,
            ProofTransport::Local,
        )
        .unwrap();

        let expired = VerifyRequest {
            envelope: &envelope,
            expected_gate: &gate,
            expected_verifier_id: &gate.context.verifier_id,
            now_unix_ms: 1_101,
        };
        assert_eq!(
            validate_claims_and_signature(&expired).unwrap_err(),
            VerificationError::ExpiredChallenge
        );

        let wrong_verifier = VerifyRequest {
            envelope: &envelope,
            expected_gate: &gate,
            expected_verifier_id: "other-verifier",
            now_unix_ms: 1_050,
        };
        assert_eq!(
            validate_claims_and_signature(&wrong_verifier).unwrap_err(),
            VerificationError::WrongVerifier
        );
    }

    #[test]
    fn proof_must_be_fresh_for_the_current_challenge() {
        let gate = gate();
        let signing_key = SigningKey::from_bytes(&[3; 32]);
        let challenge = VerificationChallenge::new(
            gate.context_hash(),
            gate.context.verifier_id.clone(),
            [5; 32],
            700_000,
            700_500,
        );
        let envelope = present(
            &proof(&signing_key),
            challenge,
            &signing_key,
            ProofTransport::Local,
        )
        .unwrap();
        let request = VerifyRequest {
            envelope: &envelope,
            expected_gate: &gate,
            expected_verifier_id: &gate.context.verifier_id,
            now_unix_ms: 700_100,
        };
        assert_eq!(
            validate_claims_and_signature(&request).unwrap_err(),
            VerificationError::InvalidProofTime
        );

        let mut future_proof = proof(&signing_key);
        future_proof.journal.issued_at_unix_ms = 100_000;
        let challenge = VerificationChallenge::new(
            gate.context_hash(),
            gate.context.verifier_id.clone(),
            [6; 32],
            1_000,
            1_500,
        );
        let envelope = present(
            &future_proof,
            challenge,
            &signing_key,
            ProofTransport::Local,
        )
        .unwrap();
        let request = VerifyRequest {
            envelope: &envelope,
            expected_gate: &gate,
            expected_verifier_id: &gate.context.verifier_id,
            now_unix_ms: 1_200,
        };
        assert_eq!(
            validate_claims_and_signature(&request).unwrap_err(),
            VerificationError::InvalidProofTime
        );
    }

    #[test]
    fn replay_cache_consumes_each_challenge_once() {
        let mut cache = InMemoryReplayCache::default();
        let digest = [7; 32];

        assert!(cache.consume(digest));
        assert!(!cache.consume(digest));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn deterministic_error_codes_are_stable() {
        assert_eq!(
            VerificationError::InvalidProof.code() as u16,
            VerificationErrorCode::InvalidProof as u16
        );
        assert_eq!(VerificationErrorCode::InvalidProof as u16, 1001);
        assert_eq!(VerificationErrorCode::BadPresenter as u16, 1009);
        assert_eq!(VerificationErrorCode::ReplayedChallenge as u16, 1010);
    }

    #[test]
    fn challenge_factory_enforces_ttl_and_verifier() {
        let gate = gate();
        let first = create_challenge(&gate, &gate.context.verifier_id, 1_000, 1_000).unwrap();
        let second = create_challenge(&gate, &gate.context.verifier_id, 1_000, 1_000).unwrap();

        assert_ne!(first.nonce, second.nonce);
        assert_eq!(
            create_challenge(&gate, "other", 1_000, 1_000).unwrap_err(),
            VerificationError::WrongVerifier
        );
        assert_eq!(
            create_challenge(
                &gate,
                &gate.context.verifier_id,
                1_000,
                MAX_CHALLENGE_TTL_MS + 1,
            )
            .unwrap_err(),
            VerificationError::InvalidChallenge
        );
    }
}

pub use attestation_image_id::BALANCE_ATTESTATION_IMAGE_ID;
use attestation_types::{
    sha256, AttestationJournal, Digest32, GateContext, ATTESTATION_JOURNAL_VERSION,
};
use borsh::{BorshDeserialize, BorshSerialize};
use ed25519_dalek::{Signature, Verifier as _, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const GATE_STATE_VERSION: u16 = 1;
pub const ACCESS_BADGE_VERSION: u16 = 1;
pub const ON_CHAIN_CHALLENGE_VERSION: u16 = 1;
pub const ON_CHAIN_CHALLENGE_DOMAIN: &[u8] = b"LEZ-TokenStudio/OnChainChallenge/v1";
pub const ON_CHAIN_PRESENTATION_DOMAIN: &[u8] = b"LEZ-TokenStudio/OnChainPresentation/v1";
pub const ON_CHAIN_NONCE_DOMAIN: &[u8] = b"LEZ-TokenStudio/OnChainNonce/v1";
pub const MAX_ON_CHAIN_PROOF_AGE_MS: u64 = 10 * 60 * 1_000;
pub const MAX_ON_CHAIN_CLOCK_SKEW_MS: u64 = 30_000;

pub type ProgramId = [u32; 8];

#[spel_framework::account_type]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct OnChainAttestationJournal {
    pub version: u16,
    pub context_hash: [u8; 32],
    pub token_program_owner: [u32; 8],
    pub token_definition_id: [u8; 32],
    pub threshold: u128,
    pub commitment_root: [u8; 32],
    pub presenter_public_key: [u8; 32],
    pub issued_at_unix_ms: u64,
    pub expires_at_unix_ms: Option<u64>,
}

impl OnChainAttestationJournal {
    #[must_use]
    pub fn to_attestation_journal(&self) -> AttestationJournal {
        AttestationJournal {
            version: self.version,
            context_hash: self.context_hash,
            token_program_owner: self.token_program_owner,
            token_definition_id: self.token_definition_id,
            threshold: self.threshold,
            commitment_root: self.commitment_root,
            presenter_public_key: self.presenter_public_key,
            issued_at_unix_ms: self.issued_at_unix_ms,
            expires_at_unix_ms: self.expires_at_unix_ms,
        }
    }

    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        self.to_attestation_journal().canonical_bytes()
    }
}

impl From<&AttestationJournal> for OnChainAttestationJournal {
    fn from(journal: &AttestationJournal) -> Self {
        Self {
            version: journal.version,
            context_hash: journal.context_hash,
            token_program_owner: journal.token_program_owner,
            token_definition_id: journal.token_definition_id,
            threshold: journal.threshold,
            commitment_root: journal.commitment_root,
            presenter_public_key: journal.presenter_public_key,
            issued_at_unix_ms: journal.issued_at_unix_ms,
            expires_at_unix_ms: journal.expires_at_unix_ms,
        }
    }
}

#[spel_framework::account_type]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct GateState {
    pub version: u16,
    pub context_hash: [u8; 32],
    pub token_program_owner: [u32; 8],
    pub token_definition_id: [u8; 32],
    pub threshold: u128,
    pub expires_at_unix_ms: Option<u64>,
    pub challenge_nonce: [u8; 32],
    pub claim_counter: u64,
}

impl GateState {
    #[must_use]
    pub fn new(context: &GateContext, challenge_nonce: Digest32) -> Self {
        Self::from_public_inputs(
            context.context_hash(),
            context.token_program_owner,
            context.token_definition_id,
            context.threshold,
            context.expires_at_unix_ms,
            challenge_nonce,
        )
    }

    #[must_use]
    pub const fn from_public_inputs(
        context_hash: [u8; 32],
        token_program_owner: [u32; 8],
        token_definition_id: [u8; 32],
        threshold: u128,
        expires_at_unix_ms: Option<u64>,
        challenge_nonce: [u8; 32],
    ) -> Self {
        Self {
            version: GATE_STATE_VERSION,
            context_hash,
            token_program_owner,
            token_definition_id,
            threshold,
            expires_at_unix_ms,
            challenge_nonce,
            claim_counter: 0,
        }
    }

    pub fn validate(&self) -> Result<(), ClaimError> {
        if self.version != GATE_STATE_VERSION
            || self.threshold == 0
            || self.challenge_nonce == [0; 32]
        {
            return Err(ClaimError::InvalidGateState);
        }
        Ok(())
    }

    #[must_use]
    pub fn challenge(&self, program_id: ProgramId, claim_account_id: Digest32) -> OnChainChallenge {
        OnChainChallenge {
            version: ON_CHAIN_CHALLENGE_VERSION,
            program_id,
            claim_account_id,
            gate_context_hash: self.context_hash,
            nonce: self.challenge_nonce,
            claim_counter: self.claim_counter,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnChainChallenge {
    pub version: u16,
    pub program_id: ProgramId,
    pub claim_account_id: [u8; 32],
    pub gate_context_hash: [u8; 32],
    pub nonce: [u8; 32],
    pub claim_counter: u64,
}

impl OnChainChallenge {
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(ON_CHAIN_CHALLENGE_DOMAIN);
        bytes.extend_from_slice(&self.version.to_le_bytes());
        for word in self.program_id {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&self.claim_account_id);
        bytes.extend_from_slice(&self.gate_context_hash);
        bytes.extend_from_slice(&self.nonce);
        bytes.extend_from_slice(&self.claim_counter.to_le_bytes());
        bytes
    }

    #[must_use]
    pub fn digest(&self) -> Digest32 {
        sha256(&self.canonical_bytes())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GateInstruction {
    Initialize {
        context_hash: [u8; 32],
        token_program_owner: [u32; 8],
        token_definition_id: [u8; 32],
        threshold: u128,
        expires_at_unix_ms: u64,
        challenge_nonce: [u8; 32],
    },
    Claim {
        claim: ClaimAccess,
    },
}

#[spel_framework::account_type]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct ClaimAccess {
    pub journal: OnChainAttestationJournal,
    pub presenter_signature: Vec<u8>,
}

#[spel_framework::account_type]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, BorshSerialize, BorshDeserialize)]
pub struct AccessBadge {
    pub version: u16,
    pub context_hash: [u8; 32],
    pub presenter_public_key: [u8; 32],
    pub claim_number: u64,
    pub proof_issued_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClaimResult {
    pub next_gate_state: GateState,
    pub badge: AccessBadge,
    pub challenge_digest: Digest32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ClaimErrorCode {
    InvalidGateState = 2000,
    MalformedClaim = 1011,
    InvalidJournal = 1001,
    WrongContext = 1002,
    WrongToken = 1003,
    WrongThreshold = 1004,
    BadPresenter = 1009,
    CounterOverflow = 2001,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ClaimError {
    #[error("gate state is malformed or unsupported")]
    InvalidGateState,
    #[error("claim payload is malformed")]
    MalformedClaim,
    #[error("attestation journal version is unsupported")]
    InvalidJournal,
    #[error("attestation context or expiry does not match the gate")]
    WrongContext,
    #[error("attestation token does not match the gate")]
    WrongToken,
    #[error("attestation threshold does not equal the gate threshold")]
    WrongThreshold,
    #[error("presenter signature is invalid")]
    BadPresenter,
    #[error("gate claim counter overflowed")]
    CounterOverflow,
}

impl ClaimError {
    #[must_use]
    pub const fn code(&self) -> ClaimErrorCode {
        match self {
            Self::InvalidGateState => ClaimErrorCode::InvalidGateState,
            Self::MalformedClaim => ClaimErrorCode::MalformedClaim,
            Self::InvalidJournal => ClaimErrorCode::InvalidJournal,
            Self::WrongContext => ClaimErrorCode::WrongContext,
            Self::WrongToken => ClaimErrorCode::WrongToken,
            Self::WrongThreshold => ClaimErrorCode::WrongThreshold,
            Self::BadPresenter => ClaimErrorCode::BadPresenter,
            Self::CounterOverflow => ClaimErrorCode::CounterOverflow,
        }
    }
}

pub fn evaluate_claim(
    state: &GateState,
    program_id: ProgramId,
    claim_account_id: Digest32,
    claim: &ClaimAccess,
) -> Result<ClaimResult, ClaimError> {
    state.validate()?;
    let journal = &claim.journal;
    if journal.version != ATTESTATION_JOURNAL_VERSION {
        return Err(ClaimError::InvalidJournal);
    }
    if journal.context_hash != state.context_hash
        || journal.expires_at_unix_ms != state.expires_at_unix_ms
    {
        return Err(ClaimError::WrongContext);
    }
    if journal.token_program_owner != state.token_program_owner
        || journal.token_definition_id != state.token_definition_id
    {
        return Err(ClaimError::WrongToken);
    }
    if journal.threshold != state.threshold {
        return Err(ClaimError::WrongThreshold);
    }

    let challenge = state.challenge(program_id, claim_account_id);
    let verifying_key = VerifyingKey::from_bytes(&journal.presenter_public_key)
        .map_err(|_| ClaimError::BadPresenter)?;
    let signature = Signature::from_slice(&claim.presenter_signature)
        .map_err(|_| ClaimError::MalformedClaim)?;
    verifying_key
        .verify(
            &on_chain_presentation_message(&challenge, journal),
            &signature,
        )
        .map_err(|_| ClaimError::BadPresenter)?;

    let claim_number = state
        .claim_counter
        .checked_add(1)
        .ok_or(ClaimError::CounterOverflow)?;
    let mut nonce_input = Vec::new();
    nonce_input.extend_from_slice(ON_CHAIN_NONCE_DOMAIN);
    nonce_input.extend_from_slice(&state.challenge_nonce);
    nonce_input.extend_from_slice(&claim.presenter_signature);

    let mut next_gate_state = state.clone();
    next_gate_state.challenge_nonce = sha256(&nonce_input);
    next_gate_state.claim_counter = claim_number;

    Ok(ClaimResult {
        next_gate_state,
        badge: AccessBadge {
            version: ACCESS_BADGE_VERSION,
            context_hash: state.context_hash,
            presenter_public_key: journal.presenter_public_key,
            claim_number,
            proof_issued_at_unix_ms: journal.issued_at_unix_ms,
        },
        challenge_digest: challenge.digest(),
    })
}

#[must_use]
pub fn on_chain_presentation_message(
    challenge: &OnChainChallenge,
    journal: &OnChainAttestationJournal,
) -> Digest32 {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(ON_CHAIN_PRESENTATION_DOMAIN);
    bytes.extend_from_slice(&challenge.canonical_bytes());
    for word in BALANCE_ATTESTATION_IMAGE_ID {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(&journal.canonical_bytes());
    sha256(&bytes)
}

pub fn encode_gate_state(state: &GateState) -> Result<Vec<u8>, std::io::Error> {
    borsh::to_vec(state)
}

pub fn decode_gate_state(bytes: &[u8]) -> Result<GateState, std::io::Error> {
    GateState::try_from_slice(bytes)
}

pub fn encode_access_badge(badge: &AccessBadge) -> Result<Vec<u8>, std::io::Error> {
    borsh::to_vec(badge)
}

pub fn decode_access_badge(bytes: &[u8]) -> Result<AccessBadge, std::io::Error> {
    AccessBadge::try_from_slice(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::AttestationJournal;
    use ed25519_dalek::{Signer as _, SigningKey};

    fn context() -> GateContext {
        GateContext {
            application_id: "tokenstudio".to_owned(),
            gate_id: "founders".to_owned(),
            token_program_owner: [7; 8],
            token_definition_id: [4; 32],
            threshold: 100,
            verifier_id: "lez:founders".to_owned(),
            expires_at_unix_ms: Some(2_000_000),
        }
    }

    fn journal(signing_key: &SigningKey) -> OnChainAttestationJournal {
        OnChainAttestationJournal::from(&AttestationJournal::new(
            &context(),
            [8; 32],
            signing_key.verifying_key().to_bytes(),
            1_000,
        ))
    }

    fn signed_claim(
        state: &GateState,
        signing_key: &SigningKey,
        program_id: ProgramId,
        claim_account_id: Digest32,
    ) -> ClaimAccess {
        let journal = journal(signing_key);
        let challenge = state.challenge(program_id, claim_account_id);
        let signature = signing_key.sign(&on_chain_presentation_message(&challenge, &journal));
        ClaimAccess {
            journal,
            presenter_signature: signature.to_bytes().to_vec(),
        }
    }

    #[test]
    fn valid_claim_rotates_nonce_and_issues_badge() {
        let state = GateState::new(&context(), [5; 32]);
        let signing_key = SigningKey::from_bytes(&[3; 32]);
        let program_id = [9; 8];
        let claim_account_id = [6; 32];
        let claim = signed_claim(&state, &signing_key, program_id, claim_account_id);

        let result = evaluate_claim(&state, program_id, claim_account_id, &claim).unwrap();

        assert_eq!(result.next_gate_state.claim_counter, 1);
        assert_ne!(
            result.next_gate_state.challenge_nonce,
            state.challenge_nonce
        );
        assert_eq!(result.badge.context_hash, state.context_hash);
        assert_eq!(
            result.badge.presenter_public_key,
            signing_key.verifying_key().to_bytes()
        );
        assert_eq!(
            decode_access_badge(&encode_access_badge(&result.badge).unwrap()).unwrap(),
            result.badge
        );
    }

    #[test]
    fn replayed_claim_fails_after_nonce_rotation() {
        let state = GateState::new(&context(), [5; 32]);
        let signing_key = SigningKey::from_bytes(&[3; 32]);
        let program_id = [9; 8];
        let claim_account_id = [6; 32];
        let claim = signed_claim(&state, &signing_key, program_id, claim_account_id);
        let first = evaluate_claim(&state, program_id, claim_account_id, &claim).unwrap();

        assert_eq!(
            evaluate_claim(&first.next_gate_state, program_id, claim_account_id, &claim,)
                .unwrap_err(),
            ClaimError::BadPresenter
        );
    }

    #[test]
    fn signature_cannot_be_forwarded_or_moved_to_another_gate() {
        let state = GateState::new(&context(), [5; 32]);
        let proof_key = SigningKey::from_bytes(&[3; 32]);
        let other_key = SigningKey::from_bytes(&[4; 32]);
        let program_id = [9; 8];
        let claim_account_id = [6; 32];
        let mut claim = signed_claim(&state, &proof_key, program_id, claim_account_id);
        claim.presenter_signature = other_key
            .sign(&on_chain_presentation_message(
                &state.challenge(program_id, claim_account_id),
                &claim.journal,
            ))
            .to_bytes()
            .to_vec();

        assert_eq!(
            evaluate_claim(&state, program_id, claim_account_id, &claim).unwrap_err(),
            ClaimError::BadPresenter
        );

        let claim = signed_claim(&state, &proof_key, program_id, claim_account_id);
        assert_eq!(
            evaluate_claim(&state, program_id, [7; 32], &claim).unwrap_err(),
            ClaimError::BadPresenter
        );
    }

    #[test]
    fn exact_context_token_and_threshold_are_enforced() {
        let state = GateState::new(&context(), [5; 32]);
        let signing_key = SigningKey::from_bytes(&[3; 32]);
        let program_id = [9; 8];
        let claim_account_id = [6; 32];

        let mut wrong_context = signed_claim(&state, &signing_key, program_id, claim_account_id);
        wrong_context.journal.context_hash[0] ^= 1;
        assert_eq!(
            evaluate_claim(&state, program_id, claim_account_id, &wrong_context).unwrap_err(),
            ClaimError::WrongContext
        );

        let mut wrong_token = signed_claim(&state, &signing_key, program_id, claim_account_id);
        wrong_token.journal.token_definition_id[0] ^= 1;
        assert_eq!(
            evaluate_claim(&state, program_id, claim_account_id, &wrong_token).unwrap_err(),
            ClaimError::WrongToken
        );

        let mut wrong_threshold = signed_claim(&state, &signing_key, program_id, claim_account_id);
        wrong_threshold.journal.threshold += 1;
        assert_eq!(
            evaluate_claim(&state, program_id, claim_account_id, &wrong_threshold).unwrap_err(),
            ClaimError::WrongThreshold
        );
    }

    #[test]
    fn gate_state_round_trip_is_stable() {
        let state = GateState::new(&context(), [5; 32]);
        assert_eq!(
            decode_gate_state(&encode_gate_state(&state).unwrap()).unwrap(),
            state
        );
    }
}

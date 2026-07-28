use std::collections::BTreeMap;

use attestation_types::{digest_to_hex, AttestationEnvelope, Digest32, VerificationChallenge};
use attestation_verifier::{
    create_challenge, verify, InMemoryReplayCache, VerificationError, VerifyRequest,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokenstudio_config::GateConfig;

pub const GOVERNANCE_STATE_VERSION: u16 = 1;
pub const GOVERNANCE_APPLICATION_ID: &str = "proofgate-governance";
pub const MAX_PENDING_CHALLENGES: usize = 4_096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VoteChoice {
    Yes,
    No,
    Abstain,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoteRecord {
    pub choice: VoteChoice,
    pub cast_at_unix_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GovernanceState {
    pub version: u16,
    pub proposal_id: String,
    pub gate_context_hash: String,
    pub pending_challenges: BTreeMap<String, VerificationChallenge>,
    pub consumed_challenges: Vec<String>,
    pub votes: BTreeMap<String, VoteRecord>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CastOutcome {
    pub presenter_public_key: Digest32,
    pub choice: VoteChoice,
    pub total_votes: usize,
}

#[derive(Debug, Error)]
pub enum GovernanceError {
    #[error("invalid gate configuration: {0}")]
    InvalidGate(String),
    #[error("proposal id must be 1..=128 visible ASCII characters")]
    InvalidProposalId,
    #[error("gate is not bound to governance proposal {proposal_id}")]
    WrongProposalBinding { proposal_id: String },
    #[error("governance state version {actual} is unsupported; expected {expected}")]
    UnsupportedStateVersion { actual: u16, expected: u16 },
    #[error("governance state does not match the supplied gate or proposal")]
    StateMismatch,
    #[error("too many outstanding verification challenges")]
    TooManyPendingChallenges,
    #[error("presentation challenge was not issued by this governance service")]
    UnknownChallenge,
    #[error("presenter has already voted on this proposal")]
    DuplicateVote,
    #[error("stored challenge digest is malformed")]
    MalformedStateDigest,
    #[error("attestation denied [{code}]: {source}")]
    Verification {
        code: u16,
        #[source]
        source: VerificationError,
    },
}

impl GovernanceState {
    pub fn new(gate: &GateConfig, proposal_id: &str) -> Result<Self, GovernanceError> {
        validate_binding(gate, proposal_id)?;
        Ok(Self {
            version: GOVERNANCE_STATE_VERSION,
            proposal_id: proposal_id.to_owned(),
            gate_context_hash: digest_to_hex(&gate.context_hash()),
            pending_challenges: BTreeMap::new(),
            consumed_challenges: Vec::new(),
            votes: BTreeMap::new(),
        })
    }

    pub fn validate(&self, gate: &GateConfig, proposal_id: &str) -> Result<(), GovernanceError> {
        if self.version != GOVERNANCE_STATE_VERSION {
            return Err(GovernanceError::UnsupportedStateVersion {
                actual: self.version,
                expected: GOVERNANCE_STATE_VERSION,
            });
        }
        validate_binding(gate, proposal_id)?;
        if self.proposal_id != proposal_id
            || self.gate_context_hash != digest_to_hex(&gate.context_hash())
        {
            return Err(GovernanceError::StateMismatch);
        }
        for digest in &self.consumed_challenges {
            parse_digest(digest)?;
        }
        for (digest, challenge) in &self.pending_challenges {
            if parse_digest(digest)? != challenge.digest() {
                return Err(GovernanceError::StateMismatch);
            }
        }
        for presenter in self.votes.keys() {
            parse_digest(presenter)?;
        }
        Ok(())
    }

    pub fn issue_challenge(
        &mut self,
        gate: &GateConfig,
        expected_commitment_root: Digest32,
        now_unix_ms: u64,
        ttl_ms: u64,
    ) -> Result<VerificationChallenge, GovernanceError> {
        self.validate(gate, &self.proposal_id)?;
        if self.pending_challenges.len() >= MAX_PENDING_CHALLENGES {
            return Err(GovernanceError::TooManyPendingChallenges);
        }
        let challenge = create_challenge(
            gate,
            expected_commitment_root,
            &gate.context.verifier_id,
            now_unix_ms,
            ttl_ms,
        )
        .map_err(verification_error)?;
        self.pending_challenges
            .insert(digest_to_hex(&challenge.digest()), challenge.clone());
        Ok(challenge)
    }

    pub fn cast_vote(
        &mut self,
        gate: &GateConfig,
        envelope: &AttestationEnvelope,
        choice: VoteChoice,
        now_unix_ms: u64,
    ) -> Result<CastOutcome, GovernanceError> {
        self.validate(gate, &self.proposal_id)?;
        let challenge_digest = envelope.challenge.digest();
        let challenge_key = digest_to_hex(&challenge_digest);
        let expected_challenge = self
            .pending_challenges
            .get(&challenge_key)
            .ok_or(GovernanceError::UnknownChallenge)?;
        let presenter_key = digest_to_hex(&envelope.journal.presenter_public_key);
        if self.votes.contains_key(&presenter_key) {
            return Err(GovernanceError::DuplicateVote);
        }

        let consumed = self
            .consumed_challenges
            .iter()
            .map(|digest| parse_digest(digest))
            .collect::<Result<Vec<_>, _>>()?;
        let mut replay_cache = InMemoryReplayCache::from_consumed(consumed);
        let verified = verify(
            VerifyRequest {
                envelope,
                expected_challenge,
                expected_gate: gate,
                expected_verifier_id: &gate.context.verifier_id,
                now_unix_ms,
            },
            &mut replay_cache,
        )
        .map_err(verification_error)?;

        self.pending_challenges.remove(&challenge_key);
        self.consumed_challenges = replay_cache.consumed().map(digest_to_hex).collect();
        self.consumed_challenges.sort_unstable();
        self.votes.insert(
            digest_to_hex(&verified.presenter_public_key),
            VoteRecord {
                choice,
                cast_at_unix_ms: now_unix_ms,
            },
        );

        Ok(CastOutcome {
            presenter_public_key: verified.presenter_public_key,
            choice,
            total_votes: self.votes.len(),
        })
    }
}

fn validate_binding(gate: &GateConfig, proposal_id: &str) -> Result<(), GovernanceError> {
    gate.validate()
        .map_err(|error| GovernanceError::InvalidGate(error.to_string()))?;
    if proposal_id.is_empty()
        || proposal_id.len() > 128
        || !proposal_id.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(GovernanceError::InvalidProposalId);
    }
    let expected_gate_id = format!("proposal:{proposal_id}");
    let expected_verifier_id = format!("proofgate-governance:{proposal_id}");
    if gate.context.application_id != GOVERNANCE_APPLICATION_ID
        || gate.context.gate_id != expected_gate_id
        || gate.context.verifier_id != expected_verifier_id
    {
        return Err(GovernanceError::WrongProposalBinding {
            proposal_id: proposal_id.to_owned(),
        });
    }
    Ok(())
}

fn parse_digest(value: &str) -> Result<Digest32, GovernanceError> {
    let decoded = hex::decode(value).map_err(|_| GovernanceError::MalformedStateDigest)?;
    decoded
        .try_into()
        .map_err(|_| GovernanceError::MalformedStateDigest)
}

fn verification_error(source: VerificationError) -> GovernanceError {
    GovernanceError::Verification {
        code: source.code() as u16,
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::{AttestationJournal, GateContext, ProofTransport};
    use tokenstudio_config::{TokenConfig, GATE_CONFIG_VERSION, TOKEN_CONFIG_VERSION};

    const PROPOSAL: &str = "treasury-7";

    fn gate() -> GateConfig {
        let token = TokenConfig {
            schema_version: TOKEN_CONFIG_VERSION,
            name: "Founders Token".to_owned(),
            symbol: "FNDR".to_owned(),
            decimals: 0,
            definition_account_id: [0x11; 32],
            token_program_owner: [0x0707_0707; 8],
            definition_account: "Public/founders-definition".to_owned(),
            supply_account: "Private/founders-supply".to_owned(),
            issuer_account: "Private/founders-issuer".to_owned(),
            total_supply: 1_000_000,
        };
        GateConfig {
            schema_version: GATE_CONFIG_VERSION,
            context: GateContext {
                application_id: GOVERNANCE_APPLICATION_ID.to_owned(),
                gate_id: format!("proposal:{PROPOSAL}"),
                token_program_owner: token.token_program_owner,
                token_definition_id: token.definition_account_id,
                threshold: 100,
                verifier_id: format!("proofgate-governance:{PROPOSAL}"),
                expires_at_unix_ms: None,
            },
            token,
        }
    }

    #[test]
    fn proposal_binding_is_mandatory() {
        let mut wrong = gate();
        wrong.context.gate_id = "proposal:other".to_owned();
        assert!(matches!(
            GovernanceState::new(&wrong, PROPOSAL),
            Err(GovernanceError::WrongProposalBinding { .. })
        ));
    }

    #[test]
    fn issued_challenge_is_root_and_proposal_bound() {
        let gate = gate();
        let mut state = GovernanceState::new(&gate, PROPOSAL).unwrap();
        let challenge = state
            .issue_challenge(&gate, [0x44; 32], 1_000, 500)
            .unwrap();

        assert_eq!(challenge.gate_context_hash, gate.context_hash());
        assert_eq!(challenge.expected_commitment_root, [0x44; 32]);
        assert_eq!(challenge.verifier_id, gate.context.verifier_id);
        assert!(state
            .pending_challenges
            .contains_key(&digest_to_hex(&challenge.digest())));
    }

    #[test]
    fn unissued_challenge_is_denied_before_receipt_verification() {
        let gate = gate();
        let mut state = GovernanceState::new(&gate, PROPOSAL).unwrap();
        let challenge = VerificationChallenge::new(
            gate.context_hash(),
            [0x44; 32],
            gate.context.verifier_id.clone(),
            [0x55; 32],
            1_000,
            1_500,
        );
        let envelope = AttestationEnvelope {
            journal: AttestationJournal::new(&gate.context, [0x44; 32], [0x66; 32], 1_000),
            receipt: vec![1, 2, 3],
            challenge,
            presenter_signature: vec![0; 64],
            transport: ProofTransport::Local,
        };

        assert!(matches!(
            state.cast_vote(&gate, &envelope, VoteChoice::Yes, 1_100),
            Err(GovernanceError::UnknownChallenge)
        ));
    }

    #[test]
    fn malformed_persisted_digest_is_rejected() {
        let gate = gate();
        let mut state = GovernanceState::new(&gate, PROPOSAL).unwrap();
        state.consumed_challenges.push("not-hex".to_owned());
        assert!(matches!(
            state.validate(&gate, PROPOSAL),
            Err(GovernanceError::MalformedStateDigest)
        ));
    }
}

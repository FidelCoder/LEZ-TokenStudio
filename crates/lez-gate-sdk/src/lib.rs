use attestation_image_id::BALANCE_ATTESTATION_IMAGE_ID;
use attestation_prover::{decode_receipt, verify_receipt, ProverError, Risc0AttestationProof};
use balance_gate_core::{
    decode_access_badge, decode_gate_state, encode_gate_state, AccessBadge, ClaimAccess,
    GateInstruction, GateState,
};
use balance_gate_methods::{BALANCE_GATE_ELF, BALANCE_GATE_ID};
use lee::{
    privacy_preserving_transaction::{
        circuit::Proof, message::Message as PrivateMessage,
        witness_set::WitnessSet as PrivateWitnessSet,
    },
    program_deployment_transaction::Message as DeploymentMessage,
    public_transaction::{Message as PublicMessage, WitnessSet as PublicWitnessSet},
    PrivacyPreservingTransaction, PrivateKey, ProgramDeploymentTransaction, PublicKey,
    PublicTransaction,
};
use lee_core::{
    account::{Account, AccountId, AccountWithMetadata, Nonce},
    program::{InstructionData, ProgramId, ProgramOutput, DEFAULT_PROGRAM_ID},
    InputAccountIdentity, PrivacyPreservingCircuitInput, PrivacyPreservingCircuitOutput,
};
use risc0_zkvm::{
    default_executor, default_prover, serde::to_vec, sha::Digestible as _, ExecutorEnv,
    MaybePruned, ProverOpts, Receipt, ReceiptClaim,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LezGateError {
    #[error("attestation proof is invalid: {0}")]
    InvalidAttestation(#[from] ProverError),
    #[error("claim journal differs from the supplied attestation receipt")]
    JournalMismatch,
    #[error("failed to serialize Risc0 input: {0}")]
    InputEncoding(String),
    #[error("failed to build Risc0 executor environment: {0}")]
    Environment(String),
    #[error("LEZ gate program execution failed: {0}")]
    ProgramExecution(String),
    #[error("LEZ gate program proving failed: {0}")]
    ProgramProving(String),
    #[error("LEZ gate program receipt is invalid: {0}")]
    ProgramVerification(String),
    #[error("LEZ gate program output is invalid: {0}")]
    ProgramOutput(String),
    #[error("LEZ outer private-execution proving failed: {0}")]
    OuterProving(String),
    #[error("LEZ outer private-execution receipt is invalid: {0}")]
    OuterVerification(String),
    #[error("LEZ outer private-execution output is invalid: {0}")]
    OuterOutput(String),
    #[error("failed to encode LEZ private-execution proof: {0}")]
    ProofEncoding(String),
    #[error("failed to construct LEZ transaction: {0}")]
    Transaction(String),
    #[error("LEZ signer resolves to {actual:?}, expected {expected:?}")]
    WrongSigner {
        expected: AccountId,
        actual: AccountId,
    },
}

#[derive(Clone, Debug)]
pub struct GateProgramExecution {
    pub output: ProgramOutput,
    pub receipt: Receipt,
}

#[derive(Debug)]
pub struct ComposedGateExecution {
    pub gate_output: ProgramOutput,
    pub circuit_output: PrivacyPreservingCircuitOutput,
    pub proof: Proof,
}

#[must_use]
pub fn signer_account_id(private_key: &PrivateKey) -> AccountId {
    AccountId::from(&PublicKey::new_from_private_key(private_key))
}

#[must_use]
pub fn gate_deployment_transaction() -> ProgramDeploymentTransaction {
    ProgramDeploymentTransaction::new(DeploymentMessage::new(BALANCE_GATE_ELF.to_vec()))
}

pub fn gate_initialization_transaction(
    state: &GateState,
    gate_account_id: AccountId,
    gate_nonce: Nonce,
    gate_private_key: &PrivateKey,
) -> Result<PublicTransaction, LezGateError> {
    ensure_signer(gate_account_id, gate_private_key)?;
    state
        .validate()
        .map_err(|error| LezGateError::Transaction(error.to_string()))?;
    let instruction = GateInstruction::Initialize {
        context_hash: state.context_hash,
        token_program_owner: state.token_program_owner,
        token_definition_id: state.token_definition_id,
        threshold: state.threshold,
        commitment_root: state.commitment_root,
        expires_at_unix_ms: state.expires_at_unix_ms.unwrap_or(0),
        challenge_nonce: state.challenge_nonce,
    };
    let message = PublicMessage::try_new(
        BALANCE_GATE_ID,
        vec![gate_account_id],
        vec![gate_nonce],
        instruction,
    )
    .map_err(|error| LezGateError::Transaction(error.to_string()))?;
    let witness_set = PublicWitnessSet::for_message(&message, &[gate_private_key]);
    Ok(PublicTransaction::new(message, witness_set))
}

pub fn signed_gate_claim_transaction(
    composed: ComposedGateExecution,
    gate_account_id: AccountId,
    gate_nonce: Nonce,
    badge_account_id: AccountId,
    badge_nonce: Nonce,
    badge_private_key: &PrivateKey,
) -> Result<PrivacyPreservingTransaction, LezGateError> {
    ensure_signer(badge_account_id, badge_private_key)?;
    let message = PrivateMessage::try_from_circuit_output(
        vec![gate_account_id, badge_account_id],
        vec![gate_nonce, badge_nonce],
        composed.circuit_output,
    )
    .map_err(|error| LezGateError::Transaction(error.to_string()))?;
    let witness_set =
        PrivateWitnessSet::for_message(&message, composed.proof, &[badge_private_key]);
    Ok(PrivacyPreservingTransaction::new(message, witness_set))
}

fn ensure_signer(expected: AccountId, private_key: &PrivateKey) -> Result<(), LezGateError> {
    let actual = signer_account_id(private_key);
    if actual != expected {
        return Err(LezGateError::WrongSigner { expected, actual });
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulatedGateClaim {
    pub next_gate_state: GateState,
    pub badge: AccessBadge,
    pub valid_from_unix_ms: Option<u64>,
    pub valid_until_unix_ms: Option<u64>,
}

#[must_use]
pub fn demo_pre_states(
    state: &GateState,
    gate_account_id: [u8; 32],
    badge_account_id: [u8; 32],
) -> Vec<AccountWithMetadata> {
    let gate = AccountWithMetadata::new(
        Account {
            program_owner: BALANCE_GATE_ID,
            data: encode_gate_state(state)
                .expect("valid gate state is Borsh serializable")
                .try_into()
                .expect("gate state fits LEZ account data"),
            ..Account::default()
        },
        false,
        AccountId::new(gate_account_id),
    );
    let badge = AccountWithMetadata::new(
        Account {
            program_owner: DEFAULT_PROGRAM_ID,
            ..Account::default()
        },
        true,
        AccountId::new(badge_account_id),
    );
    vec![gate, badge]
}

pub fn simulate_demo_claim(
    state: &GateState,
    gate_account_id: [u8; 32],
    badge_account_id: [u8; 32],
    claim: ClaimAccess,
) -> Result<SimulatedGateClaim, LezGateError> {
    let output = execute_gate_claim_conditionally(
        demo_pre_states(state, gate_account_id, badge_account_id),
        claim,
    )?;
    let next_gate_state = decode_gate_state(output.post_states[0].account().data.as_ref())
        .map_err(|error| LezGateError::ProgramOutput(error.to_string()))?;
    let badge = decode_access_badge(output.post_states[1].account().data.as_ref())
        .map_err(|error| LezGateError::ProgramOutput(error.to_string()))?;
    Ok(SimulatedGateClaim {
        next_gate_state,
        badge,
        valid_from_unix_ms: output.timestamp_validity_window.start(),
        valid_until_unix_ms: output.timestamp_validity_window.end(),
    })
}

pub fn prove_and_compose_demo_claim(
    state: &GateState,
    gate_account_id: [u8; 32],
    badge_account_id: [u8; 32],
    claim: ClaimAccess,
    attestation: &Risc0AttestationProof,
) -> Result<ComposedGateExecution, LezGateError> {
    let gate_execution =
        prove_demo_gate_claim(state, gate_account_id, badge_account_id, claim, attestation)?;
    compose_demo_gate_execution(gate_execution)
}

pub fn prove_demo_gate_claim(
    state: &GateState,
    gate_account_id: [u8; 32],
    badge_account_id: [u8; 32],
    claim: ClaimAccess,
    attestation: &Risc0AttestationProof,
) -> Result<GateProgramExecution, LezGateError> {
    prove_gate_claim(
        demo_pre_states(state, gate_account_id, badge_account_id),
        claim,
        attestation,
    )
}

pub fn compose_demo_gate_execution(
    gate_execution: GateProgramExecution,
) -> Result<ComposedGateExecution, LezGateError> {
    compose_private_execution(
        gate_execution,
        vec![InputAccountIdentity::Public, InputAccountIdentity::Public],
    )
}

pub fn execute_gate_claim_conditionally(
    pre_states: Vec<AccountWithMetadata>,
    claim: ClaimAccess,
) -> Result<ProgramOutput, LezGateError> {
    let assumption = attestation_claim(&claim.journal)?;
    let env = gate_environment(pre_states, &GateInstruction::Claim { claim }, assumption)?;
    let session = default_executor()
        .execute(env, BALANCE_GATE_ELF)
        .map_err(|error| LezGateError::ProgramExecution(error.to_string()))?;
    session
        .journal
        .decode()
        .map_err(|error| LezGateError::ProgramOutput(error.to_string()))
}

pub fn prove_gate_claim(
    pre_states: Vec<AccountWithMetadata>,
    claim: ClaimAccess,
    attestation: &Risc0AttestationProof,
) -> Result<GateProgramExecution, LezGateError> {
    let verified_journal = verify_receipt(attestation)?;
    if verified_journal != claim.journal.to_attestation_journal() {
        return Err(LezGateError::JournalMismatch);
    }
    let attestation_receipt = decode_receipt(&attestation.receipt)?;
    let env = gate_environment(
        pre_states,
        &GateInstruction::Claim { claim },
        attestation_receipt,
    )?;
    let receipt = default_prover()
        .prove(env, BALANCE_GATE_ELF)
        .map_err(|error| LezGateError::ProgramProving(error.to_string()))?
        .receipt;
    receipt
        .verify(BALANCE_GATE_ID)
        .map_err(|error| LezGateError::ProgramVerification(error.to_string()))?;
    let output = receipt
        .journal
        .decode()
        .map_err(|error| LezGateError::ProgramOutput(error.to_string()))?;
    Ok(GateProgramExecution { output, receipt })
}

pub fn compose_private_execution(
    gate_execution: GateProgramExecution,
    account_identities: Vec<InputAccountIdentity>,
) -> Result<ComposedGateExecution, LezGateError> {
    let mut env_builder = ExecutorEnv::builder();
    env_builder.add_assumption(gate_execution.receipt.clone());
    env_builder
        .write(&PrivacyPreservingCircuitInput {
            program_outputs: vec![gate_execution.output.clone()],
            account_identities,
            program_id: BALANCE_GATE_ID,
        })
        .map_err(|error| LezGateError::InputEncoding(error.to_string()))?;
    let env = env_builder
        .build()
        .map_err(|error| LezGateError::Environment(error.to_string()))?;
    let receipt = default_prover()
        .prove_with_opts(
            env,
            lee::PRIVACY_PRESERVING_CIRCUIT_ELF,
            &ProverOpts::succinct(),
        )
        .map_err(|error| LezGateError::OuterProving(error.to_string()))?
        .receipt;
    receipt
        .verify(lee::PRIVACY_PRESERVING_CIRCUIT_ID)
        .map_err(|error| LezGateError::OuterVerification(error.to_string()))?;
    let circuit_output = receipt
        .journal
        .decode()
        .map_err(|error| LezGateError::OuterOutput(error.to_string()))?;
    let proof_bytes = borsh::to_vec(&receipt.inner)
        .map_err(|error| LezGateError::ProofEncoding(error.to_string()))?;

    Ok(ComposedGateExecution {
        gate_output: gate_execution.output,
        circuit_output,
        proof: Proof::from_inner(proof_bytes),
    })
}

pub fn prove_and_compose_gate_claim(
    pre_states: Vec<AccountWithMetadata>,
    account_identities: Vec<InputAccountIdentity>,
    claim: ClaimAccess,
    attestation: &Risc0AttestationProof,
) -> Result<ComposedGateExecution, LezGateError> {
    let gate_execution = prove_gate_claim(pre_states, claim, attestation)?;
    compose_private_execution(gate_execution, account_identities)
}

fn gate_environment(
    pre_states: Vec<AccountWithMetadata>,
    instruction: &GateInstruction,
    assumption: impl Into<risc0_zkvm::AssumptionReceipt>,
) -> Result<ExecutorEnv<'static>, LezGateError> {
    let instruction_data =
        to_vec(instruction).map_err(|error| LezGateError::InputEncoding(error.to_string()))?;
    let mut env_builder = ExecutorEnv::builder();
    env_builder.add_assumption(assumption);
    write_program_inputs(
        BALANCE_GATE_ID,
        None,
        &pre_states,
        &instruction_data,
        &mut env_builder,
    )?;
    env_builder
        .build()
        .map_err(|error| LezGateError::Environment(error.to_string()))
}

fn write_program_inputs(
    program_id: ProgramId,
    caller_program_id: Option<ProgramId>,
    pre_states: &[AccountWithMetadata],
    instruction_data: &InstructionData,
    env_builder: &mut risc0_zkvm::ExecutorEnvBuilder<'_>,
) -> Result<(), LezGateError> {
    env_builder
        .write(&program_id)
        .and_then(|builder| builder.write(&caller_program_id))
        .and_then(|builder| builder.write(&pre_states.to_vec()))
        .and_then(|builder| builder.write(instruction_data))
        .map_err(|error| LezGateError::InputEncoding(error.to_string()))?;
    Ok(())
}

fn attestation_claim(
    journal: &balance_gate_core::OnChainAttestationJournal,
) -> Result<ReceiptClaim, LezGateError> {
    let attestation_journal = journal.to_attestation_journal();
    let journal_words = to_vec(&attestation_journal)
        .map_err(|error| LezGateError::InputEncoding(error.to_string()))?;
    let mut journal_bytes = Vec::with_capacity(journal_words.len() * 4);
    for word in journal_words {
        journal_bytes.extend_from_slice(&word.to_le_bytes());
    }
    Ok(ReceiptClaim::ok(
        BALANCE_ATTESTATION_IMAGE_ID,
        MaybePruned::Pruned(journal_bytes.as_slice().digest()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::{AttestationJournal, GateContext};
    use balance_gate_core::{
        decode_access_badge, decode_gate_state, encode_gate_state, on_chain_presentation_message,
        GateState,
    };
    use ed25519_dalek::{Signer as _, SigningKey};
    use lee_core::{
        account::{Account, AccountId, AccountWithMetadata},
        program::{Claim, DEFAULT_PROGRAM_ID},
    };

    fn fixture() -> (Vec<AccountWithMetadata>, ClaimAccess) {
        let context = GateContext {
            application_id: "tokenstudio".to_owned(),
            gate_id: "founders".to_owned(),
            token_program_owner: [7; 8],
            token_definition_id: [4; 32],
            threshold: 100,
            verifier_id: "lez:founders".to_owned(),
            expires_at_unix_ms: Some(2_000_000),
        };
        let state = GateState::new(&context, [9; 32], [5; 32]);
        let gate_account_id = AccountId::new([6; 32]);
        let badge_account_id = AccountId::new([8; 32]);
        let gate = AccountWithMetadata::new(
            Account {
                program_owner: BALANCE_GATE_ID,
                data: encode_gate_state(&state).unwrap().try_into().unwrap(),
                ..Account::default()
            },
            false,
            gate_account_id,
        );
        let badge = AccountWithMetadata::new(
            Account {
                program_owner: DEFAULT_PROGRAM_ID,
                ..Account::default()
            },
            true,
            badge_account_id,
        );
        let signing_key = SigningKey::from_bytes(&[3; 32]);
        let journal = AttestationJournal::new(
            &context,
            [9; 32],
            signing_key.verifying_key().to_bytes(),
            1_000,
        );
        let challenge = state.challenge(BALANCE_GATE_ID, badge_account_id.into_value());
        let on_chain_journal = balance_gate_core::OnChainAttestationJournal::from(&journal);
        let signature = signing_key.sign(&on_chain_presentation_message(
            &challenge,
            &on_chain_journal,
        ));
        (
            vec![gate, badge],
            ClaimAccess {
                journal: on_chain_journal,
                presenter_signature: signature.to_bytes().to_vec(),
            },
        )
    }

    #[test]
    fn shared_attestation_image_id_matches_generated_method() {
        assert_eq!(
            BALANCE_ATTESTATION_IMAGE_ID,
            balance_attestation_methods::BALANCE_ATTESTATION_ID
        );
    }

    #[test]
    fn initialization_requests_protocol_ownership_without_mutating_owner() {
        let state =
            GateState::from_public_inputs([2; 32], [3; 8], [4; 32], 100, [9; 32], None, [5; 32]);
        let instruction = GateInstruction::Initialize {
            context_hash: state.context_hash,
            token_program_owner: state.token_program_owner,
            token_definition_id: state.token_definition_id,
            threshold: state.threshold,
            commitment_root: state.commitment_root,
            expires_at_unix_ms: 0,
            challenge_nonce: state.challenge_nonce,
        };
        let instruction_data = to_vec(&instruction).unwrap();
        let pre_states = vec![AccountWithMetadata::new(
            Account::default(),
            true,
            AccountId::new([6; 32]),
        )];
        let mut env_builder = ExecutorEnv::builder();
        write_program_inputs(
            BALANCE_GATE_ID,
            None,
            &pre_states,
            &instruction_data,
            &mut env_builder,
        )
        .unwrap();
        let session = default_executor()
            .execute(env_builder.build().unwrap(), BALANCE_GATE_ELF)
            .unwrap();
        let output: ProgramOutput = session.journal.decode().unwrap();

        assert_eq!(
            output.post_states[0].account().program_owner,
            DEFAULT_PROGRAM_ID
        );
        assert_eq!(
            output.post_states[0].required_claim(),
            Some(Claim::Authorized)
        );
        assert_eq!(
            decode_gate_state(output.post_states[0].account().data.as_ref()).unwrap(),
            state
        );
    }

    #[test]
    fn conditional_executor_runs_real_lez_gate_logic() {
        let (pre_states, claim) = fixture();
        let output = execute_gate_claim_conditionally(pre_states, claim).unwrap();

        assert_eq!(
            output.post_states[0].account().program_owner,
            BALANCE_GATE_ID
        );
        assert_eq!(
            output.post_states[1].account().program_owner,
            DEFAULT_PROGRAM_ID
        );
        let state = decode_gate_state(output.post_states[0].account().data.as_ref()).unwrap();
        let badge = decode_access_badge(output.post_states[1].account().data.as_ref()).unwrap();
        assert_eq!(state.claim_counter, 1);
        assert_eq!(badge.claim_number, 1);
        assert_eq!(output.timestamp_validity_window.start(), Some(0));
        assert_eq!(output.timestamp_validity_window.end(), Some(601_001));
    }

    #[test]
    fn wrong_presenter_signature_fails_before_receipt_composition() {
        let (pre_states, mut claim) = fixture();
        claim.presenter_signature[0] ^= 1;

        let error = execute_gate_claim_conditionally(pre_states, claim).unwrap_err();
        assert!(error
            .to_string()
            .contains("LEZ gate program execution failed"));
    }

    #[test]
    fn initialization_transaction_is_signed_by_the_gate_account() {
        let private_key = PrivateKey::try_new([1; 32]).unwrap();
        let gate_account_id = signer_account_id(&private_key);
        let state =
            GateState::from_public_inputs([2; 32], [3; 8], [4; 32], 100, [9; 32], None, [5; 32]);

        let tx =
            gate_initialization_transaction(&state, gate_account_id, 7_u128.into(), &private_key)
                .unwrap();

        assert_eq!(tx.message().account_ids, vec![gate_account_id]);
        assert!(tx.witness_set().is_valid_for(tx.message()));
    }

    #[test]
    fn signed_claim_transaction_rejects_a_different_badge_signer() {
        let badge_private_key = PrivateKey::try_new([1; 32]).unwrap();
        let different_badge_id = signer_account_id(&PrivateKey::try_new([2; 32]).unwrap());
        let error = ensure_signer(different_badge_id, &badge_private_key).unwrap_err();

        assert!(matches!(error, LezGateError::WrongSigner { .. }));
    }
}

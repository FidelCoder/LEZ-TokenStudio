#![no_main]

use balance_gate_core::{
    decode_gate_state, encode_access_badge, encode_gate_state, evaluate_claim, ClaimAccess,
    ClaimError, GateState, BALANCE_ATTESTATION_IMAGE_ID, MAX_ON_CHAIN_CLOCK_SKEW_MS,
    MAX_ON_CHAIN_PROOF_AGE_MS,
};
use nssa_core::{
    account::AccountWithMetadata,
    program::{AccountPostState, Claim},
};
use risc0_zkvm::{guest::env, serde::to_vec};
use spel_framework::prelude::*;

risc0_zkvm::guest::entry!(main);

#[lez_program(instruction = "balance_gate_core::GateInstruction")]
mod balance_gate {
    #[allow(unused_imports)]
    use super::*;

    #[instruction]
    pub fn initialize(
        _ctx: ProgramContext,
        #[account(init, signer)] mut gate: AccountWithMetadata,
        context_hash: [u8; 32],
        token_program_owner: [u32; 8],
        token_definition_id: [u8; 32],
        threshold: u128,
        commitment_root: [u8; 32],
        expires_at_unix_ms: u64,
        challenge_nonce: [u8; 32],
    ) -> SpelResult {
        let state = GateState::from_public_inputs(
            context_hash,
            token_program_owner,
            token_definition_id,
            threshold,
            commitment_root,
            (expires_at_unix_ms != 0).then_some(expires_at_unix_ms),
            challenge_nonce,
        );
        state.validate().unwrap_or_else(|error| fail_claim(error));
        gate.account.data = encode_gate_state(&state)
            .unwrap_or_else(|_| fail(2005, "cannot encode gate state"))
            .try_into()
            .unwrap_or_else(|_| fail(2005, "gate state is too large"));

        let mut output = SpelOutput::empty();
        output.post_states = vec![AccountPostState::new_claimed(
            gate.account,
            Claim::Authorized,
        )];
        Ok(output)
    }

    #[instruction]
    pub fn claim(
        ctx: ProgramContext,
        #[account(mut, owner = self_program_id)] mut gate: AccountWithMetadata,
        #[account(init, signer)] mut badge: AccountWithMetadata,
        claim: ClaimAccess,
    ) -> SpelResult {
        let state = decode_gate_state(&gate.account.data)
            .unwrap_or_else(|_| fail(2005, "cannot decode gate state"));
        let result = evaluate_claim(
            &state,
            ctx.self_program_id,
            badge.account_id.into_value(),
            &claim,
        )
        .unwrap_or_else(|error| fail_claim(error));

        let attestation_journal = claim.journal.to_attestation_journal();
        let journal_words = to_vec(&attestation_journal)
            .unwrap_or_else(|_| fail(1011, "cannot encode attestation journal"));
        env::verify(BALANCE_ATTESTATION_IMAGE_ID, &journal_words)
            .expect("Risc0 receipt verification is infallible inside the guest");

        gate.account.data = encode_gate_state(&result.next_gate_state)
            .unwrap_or_else(|_| fail(2005, "cannot encode gate state"))
            .try_into()
            .unwrap_or_else(|_| fail(2005, "gate state is too large"));
        badge.account.data = encode_access_badge(&result.badge)
            .unwrap_or_else(|_| fail(2005, "cannot encode access badge"))
            .try_into()
            .unwrap_or_else(|_| fail(2005, "access badge is too large"));

        let valid_from = claim
            .journal
            .issued_at_unix_ms
            .saturating_sub(MAX_ON_CHAIN_CLOCK_SKEW_MS);
        let freshness_end = claim
            .journal
            .issued_at_unix_ms
            .checked_add(MAX_ON_CHAIN_PROOF_AGE_MS)
            .and_then(|value| value.checked_add(1))
            .unwrap_or_else(|| fail(1005, "proof validity overflows"));
        let valid_until = claim
            .journal
            .expires_at_unix_ms
            .and_then(|expiry| expiry.checked_add(1))
            .map_or(freshness_end, |expiry| expiry.min(freshness_end));
        if valid_from >= valid_until {
            fail(1005, "proof validity window is empty");
        }

        let mut output = SpelOutput::empty();
        output.post_states = vec![
            AccountPostState::new(gate.account),
            AccountPostState::new_claimed(badge.account, Claim::Authorized),
        ];
        output
            .try_with_timestamp_validity_window(valid_from..valid_until)
            .map_err(|_| SpelError::custom(1005, "proof validity window is invalid"))
    }
}

fn fail_claim(error: ClaimError) -> ! {
    fail(error.code() as u16, &error.to_string())
}

fn fail(code: u16, message: &str) -> ! {
    panic!("ProofGate error {code}: {message}")
}

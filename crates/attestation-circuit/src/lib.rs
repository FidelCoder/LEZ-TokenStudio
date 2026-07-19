use attestation_types::{AttestationJournal, Digest32, GateContext, PublicKey32};
use lez_compat::{FungibleTokenHolding, LezAccount, MembershipProof, TokenHoldingDecodeError};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BalanceAttestationWitness {
    pub account: LezAccount,
    pub membership_proof: MembershipProof,
    pub commitment_root: Digest32,
    pub gate_context: GateContext,
    pub presenter_public_key: PublicKey32,
    pub issued_at_unix_ms: u64,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum CircuitError {
    #[error("gate threshold must be greater than zero")]
    ZeroThreshold,
    #[error("presenter public key cannot be all zero")]
    MissingPresenterKey,
    #[error("attestation was issued after the gate expiry")]
    IssuedAfterExpiry,
    #[error("account is not owned by the configured token program")]
    WrongProgramOwner,
    #[error("account data is not a valid fungible token holding")]
    InvalidTokenHolding,
    #[error("token holding belongs to a different token definition")]
    WrongTokenDefinition,
    #[error("private token balance is below the gate threshold")]
    InsufficientBalance,
    #[error("private account commitment is not a member of the supplied root")]
    InvalidMembershipProof,
}

#[must_use = "the circuit result must be checked"]
pub fn evaluate(witness: &BalanceAttestationWitness) -> Result<AttestationJournal, CircuitError> {
    if witness.gate_context.threshold == 0 {
        return Err(CircuitError::ZeroThreshold);
    }
    if witness.presenter_public_key == [0; 32] {
        return Err(CircuitError::MissingPresenterKey);
    }
    if witness
        .gate_context
        .expires_at_unix_ms
        .is_some_and(|expiry| witness.issued_at_unix_ms > expiry)
    {
        return Err(CircuitError::IssuedAfterExpiry);
    }
    if witness.account.program_owner != witness.gate_context.token_program_owner {
        return Err(CircuitError::WrongProgramOwner);
    }

    let holding = FungibleTokenHolding::decode(&witness.account.data)
        .map_err(|_error: TokenHoldingDecodeError| CircuitError::InvalidTokenHolding)?;
    if holding.definition_id != witness.gate_context.token_definition_id {
        return Err(CircuitError::WrongTokenDefinition);
    }
    if holding.balance < witness.gate_context.threshold {
        return Err(CircuitError::InsufficientBalance);
    }

    let commitment = witness.account.commitment();
    if !witness
        .membership_proof
        .verifies(&commitment, &witness.commitment_root)
    {
        return Err(CircuitError::InvalidMembershipProof);
    }

    Ok(AttestationJournal::new(
        &witness.gate_context,
        witness.commitment_root,
        witness.presenter_public_key,
        witness.issued_at_unix_ms,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use attestation_types::{digest_to_hex, program_owner_to_hex};
    use lez_compat::Commitment;

    fn witness_with_balance(balance: u128) -> BalanceAttestationWitness {
        let definition_id = [0x44; 32];
        let account = LezAccount {
            account_id: [0x22; 32],
            program_owner: [
                0x0302_0100,
                0x0706_0504,
                0x0b0a_0908,
                0x0f0e_0d0c,
                0x1312_1110,
                0x1716_1514,
                0x1b1a_1918,
                0x1f1e_1d1c,
            ],
            balance: 9,
            nonce: 12,
            data: FungibleTokenHolding {
                definition_id,
                balance,
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
            gate_context: GateContext {
                application_id: "tokenstudio".to_owned(),
                gate_id: "founders-chat".to_owned(),
                token_program_owner: account.program_owner,
                token_definition_id: definition_id,
                threshold: 100,
                verifier_id: "logos-chat:founders".to_owned(),
                expires_at_unix_ms: Some(1_900_000_000_000),
            },
            account,
            membership_proof,
            commitment_root,
            presenter_public_key: [0x55; 32],
            issued_at_unix_ms: 1_800_000_000_000,
        }
    }

    #[test]
    fn accepts_valid_private_fungible_balance() {
        let witness = witness_with_balance(250);
        let journal = evaluate(&witness).unwrap();

        assert_eq!(journal.context_hash, witness.gate_context.context_hash());
        assert_eq!(journal.threshold, 100);
        assert_eq!(journal.token_definition_id, [0x44; 32]);
        assert_eq!(journal.commitment_root, witness.commitment_root);
        assert_eq!(journal.presenter_public_key, [0x55; 32]);
    }

    #[test]
    fn journal_does_not_expose_private_account_fields_or_exact_balance() {
        let witness = witness_with_balance(987_654_321);
        let journal = evaluate(&witness).unwrap();
        let json = serde_json::to_string(&journal).unwrap();
        let value = serde_json::to_value(&journal).unwrap();
        let fields = value.as_object().unwrap();

        for private_field in ["account_id", "balance", "nonce", "data"] {
            assert!(!fields.contains_key(private_field));
        }
        assert!(!json.contains(&digest_to_hex(&witness.account.account_id)));
        assert!(!json.contains("987654321"));
        assert!(json.contains(&program_owner_to_hex(
            &witness.gate_context.token_program_owner
        )));
    }

    #[test]
    fn rejects_insufficient_token_balance() {
        assert_eq!(
            evaluate(&witness_with_balance(99)).unwrap_err(),
            CircuitError::InsufficientBalance
        );
    }

    #[test]
    fn rejects_wrong_token_program() {
        let mut witness = witness_with_balance(100);
        witness.account.program_owner[0] ^= 1;

        assert_eq!(
            evaluate(&witness).unwrap_err(),
            CircuitError::WrongProgramOwner
        );
    }

    #[test]
    fn rejects_wrong_token_definition() {
        let mut witness = witness_with_balance(100);
        witness.gate_context.token_definition_id[0] ^= 1;

        assert_eq!(
            evaluate(&witness).unwrap_err(),
            CircuitError::WrongTokenDefinition
        );
    }

    #[test]
    fn rejects_wrong_membership_path() {
        let mut witness = witness_with_balance(100);
        witness.commitment_root[0] ^= 1;

        assert_eq!(
            evaluate(&witness).unwrap_err(),
            CircuitError::InvalidMembershipProof
        );
    }

    #[test]
    fn rejects_context_changes_that_invalidate_the_statement() {
        let mut wrong_threshold = witness_with_balance(100);
        wrong_threshold.gate_context.threshold = 101;
        assert_eq!(
            evaluate(&wrong_threshold).unwrap_err(),
            CircuitError::InsufficientBalance
        );

        let mut expired = witness_with_balance(100);
        expired.gate_context.expires_at_unix_ms = Some(expired.issued_at_unix_ms - 1);
        assert_eq!(
            evaluate(&expired).unwrap_err(),
            CircuitError::IssuedAfterExpiry
        );
    }

    #[test]
    fn rejects_malformed_or_non_fungible_data() {
        let mut malformed = witness_with_balance(100);
        malformed.account.data = vec![0; 48];
        assert_eq!(
            evaluate(&malformed).unwrap_err(),
            CircuitError::InvalidTokenHolding
        );

        let mut nft = witness_with_balance(100);
        nft.account.data[0] = 1;
        assert_eq!(
            evaluate(&nft).unwrap_err(),
            CircuitError::InvalidTokenHolding
        );
    }

    #[test]
    fn changing_private_native_balance_changes_membership_commitment() {
        let witness = witness_with_balance(100);
        let original = witness.account.commitment();
        let mut changed = witness.account;
        changed.balance += 1;

        assert_ne!(changed.commitment(), original);
        assert_ne!(
            changed.commitment().leaf_hash(),
            Commitment(*original.as_bytes()).leaf_hash()
        );
    }
}

use std::{env, path::PathBuf};

use attestation_prover::{write_prover_input, ProverInputFile};
use attestation_types::program_owner_from_hex;
use lez_compat::{FungibleTokenHolding, LezAccount, MembershipProof};

fn main() {
    let output = env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("examples/witnesses/founders.json"));
    let definition_id = [0x11; 32];
    let account = LezAccount {
        account_id: [0x22; 32],
        program_owner: program_owner_from_hex(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .expect("demo program owner is valid"),
        balance: 5,
        nonce: 9,
        data: FungibleTokenHolding {
            definition_id,
            balance: 250,
        }
        .encode()
        .to_vec(),
    };
    let membership_proof = MembershipProof {
        leaf_index: 0,
        siblings: Vec::new(),
    };
    let commitment_root = membership_proof.compute_root(&account.commitment());
    let input = ProverInputFile {
        account,
        membership_proof,
        commitment_root,
    };

    write_prover_input(&output, &input).expect("failed to write demo prover input");
    println!("Demo prover input written to {}", output.display());
}

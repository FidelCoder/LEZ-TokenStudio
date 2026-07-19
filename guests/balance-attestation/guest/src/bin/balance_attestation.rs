use attestation_circuit::{BalanceAttestationWitness, evaluate};
use risc0_zkvm::guest::env;

fn main() {
    let witness: BalanceAttestationWitness = env::read();
    let journal = evaluate(&witness).expect("invalid private balance attestation witness");
    env::commit(&journal);
}

# LEZ Compatibility Contract

ProofGate mirrors two primitives from Logos Execution Zone v0.3. These byte
layouts are consensus-sensitive inputs to the balance-attestation guest.

## Private Account Commitment

The implementation in `crates/lez-compat` mirrors
`lee/state_machine/core/src/commitment.rs::Commitment::new` from the official
`logos-execution-zone` repository:

```text
SHA256(
  "/LEE/v0.3/Commitment/" padded to 32 bytes
  || account_id[32]
  || program_owner[8] as little-endian u32 words
  || balance as little-endian u128
  || nonce as little-endian u128
  || SHA256(account_data)
)
```

The complete preimage is 160 bytes. The test suite locks down the official
all-zero `DUMMY_COMMITMENT` and `DUMMY_COMMITMENT_HASH` vectors.

## Commitment Membership Root

The implementation also mirrors `compute_digest_for_path` from the same LEZ
source file. A commitment becomes a Merkle leaf by hashing it once more. Each
sibling is then combined according to the current bit of the leaf index and
hashed again. The leaf index shifts right after each level.

This extra leaf hash is intentional and must not be removed. ProofGate verifies
the calculated root against the root returned with the sequencer membership
proof before creating an attestation.

## Upgrade Rule

Any future LEZ commitment-version change requires a new compatibility version,
new test vectors, and a new balance-attestation circuit image ID. Existing
proofs must continue to identify the circuit version that created them.

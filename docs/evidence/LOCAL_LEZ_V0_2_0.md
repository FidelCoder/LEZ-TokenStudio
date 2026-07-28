# Local LEZ v0.2.0 Evidence

Date: 2026-07-28

This record covers the final ProofGate program against an exact standalone
Logos Execution Zone node built from tag `v0.2.0`, commit `a58fbce2`. It
includes a complete real recursive private claim, not only deployment and
initialization.

## Compatibility

- `checkHealth` succeeded.
- `getAccount`, `sendTransaction`, and `getTransaction` were exercised.
- LEZ `v0.2.0` does not expose the newer `getProofsAndRoot` method.
- ProofGate's `getProofForCommitment` fallback returned a membership path and
  derived the root through `MembershipProof::compute_root`.
- The ignored live compatibility test
  `pinned_v0_2_sequencer_membership_fallback` passed.
- The sequencer accepted the final program deployment, GateState v3
  initialization, and canonical private claim transaction.

## Included Lifecycle

| Item | Value |
| --- | --- |
| Sequencer binary SHA-256 | `db06e79dc45cab873d487fad441e26444771e71fafcfa618c4cbda294bc7c7a4` |
| Proof-producing CLI SHA-256 | `b5ab45713f1f8477ee471764256b1abb2545f68f3d1f469cf511916f19837d91` |
| Program ID | `a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa` |
| Default-anchor queried root | `d4e0961e5e2774178dad46069bfb7cd610b883417f7ce07475c9c61d3b45575f` |
| Fixture proof/gate root | `510dbd3aa09bae25bc9e683e65ff2104eba32565f9c10b91582596955505b778` |
| Gate account | `9956ee9d8da1563b6f287dd6ecce79d2c901a57f09a532466c7f6a13d8fbc03c` |
| Private badge account ID | `4cb98cd381d1f8b95064843becf9aef06a4344fa085aec52023236f75274b679` |
| Deployment transaction | `7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1` |
| Initialization transaction | `056e5a2354c60c2bd69f5288ccf88855c2882abc0c443f63638af6c0067d6945` |
| Private claim transaction | `d92dc10b87e7b33891a76c80b2796cfdf736b6fd25b749e8a5de2282477d512b` |
| GateState transition | version 3, counter 0 to 1, nonce rotated |
| Badge result | private claim 1 |
| Replay result | stale claim denied |
| Restart result | all 3 transactions queryable; exact final state matched |

The deterministic witness has root
`510dbd3aa09bae25bc9e683e65ff2104eba32565f9c10b91582596955505b778`.
The gate was explicitly initialized with that authorized root. The independently
queried default-anchor root is recorded separately as legacy RPC evidence and
was not falsely substituted for the proof root.

## Private Transaction Shape

Before submission, the producing CLI enforced the exact canonical result:

| Output | Count |
| --- | ---: |
| Public post-states | 1 |
| Encrypted private post-states | 1 |
| Private commitments | 1 |
| Initialization nullifiers | 1 |

Only the gate account is a public state update. The AccessBadge plaintext and
ML-KEM secret key remain holder-local. The sequencer receives an encrypted
private post-state, its commitment, and an initialization nullifier. Private
account authorization is proven in the PPE path rather than exposed as a public
transaction signature.

## Real Proof Measurements

| Measurement | Result |
| --- | ---: |
| Balance receipt | 223,970 bytes |
| LEZ PPE proof | 230,611 bytes |
| Gate proving | 3,527,334 ms |
| Outer PPE proving | 2,191,056 ms |
| Total composition | 5,718,392 ms |
| Gate total/user/paging cycles | 3,670,016 / 3,295,256 / 168,939 |
| Gate segments | 4 |
| Outer total/user/paging cycles | 1,048,576 / 779,413 / 85,755 |
| Outer segments | 1 |

The local unaccelerated lifecycle used a six-hour GateState v3 proof-age value.
The default remains ten minutes and configuration is bounded from one minute to
24 hours.

## Persistence And Assertion Note

The claim was already included and stale replay had already been denied when
the original wrapper reached its last assertion. That assertion compared the
badge presenter's JSON byte array directly with the proof's hex string and
returned status 1 despite the successful lifecycle. The wrapper now normalizes
the byte array to lowercase hex.

All post-inclusion assertions were rerun successfully against the preserved
artifacts. The exact sequencer was then restarted on its preserved RocksDB
database. All three transaction IDs remained queryable, and the refetched
GateState exactly matched the prior final state at counter 1.

## Artifact Integrity

Public evidence is under
`artifacts/local-submission-final/standalone-claim`. Its `SHA256SUMS`
manifest covers the real balance proof, 230,611-byte LEZ proof, initial and
final state, holder-local public badge artifact, lifecycle log, results, and
validation notes.

Key hashes:

| Artifact | SHA-256 |
| --- | --- |
| Balance proof JSON | `8d6aac0a96e56b6146d368fa77be910e2a30d4cb71364af656080930bbe98f2e` |
| LEZ PPE proof | `3b85794b49f2ef764e0c88faad2f8f7c94fba27b3bfec0fa073046ca4844c64a` |
| Final GateState | `ed143f15a899ac78d4ff6e0443316080337ecc91014457a0573a1f60bb2f80ca` |
| AccessBadge | `edd3f3ea19193e91b6c4db387538e7e84ee706cbc47f2c54f4344b1f0dfa56d6` |

No presenter key, private badge key, wallet witness, or RocksDB database is
included.

## Reproduction

`scripts/ci-standalone-claim.sh` starts a clean exact-version node, checks
legacy membership compatibility, creates a real receipt, deploys and
initializes the embedded program, recursively composes the official LEZ private
execution proof, submits the private transaction, polls inclusion, fetches the
authoritative state, checks the private badge, denies stale replay, and exports
sanitized evidence.

```bash
scripts/ci-standalone-claim.sh \
  /path/to/lez-v0.2.0/sequencer_service \
  /path/to/lez-v0.2.0/sequencer_config.json
```

## Boundary

This proves the complete local exact-version lifecycle, including the real
cryptographic private claim. It is not represented as public testnet
deployment. The standalone node exposes no supported network fee metric, so
the measured guest cycles and host time are not represented as transaction
cost.

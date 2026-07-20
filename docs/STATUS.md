# Implementation Status

## Implemented And Locally Validated

- Token select/create/mint configuration and wallet command adapters.
- Deterministic, asset-specific gate configuration.
- Exact LEZ account commitment, token data, and Merkle semantics.
- Risc0 guest checks for membership, token, threshold, and context.
- Real `RISC0_DEV_MODE=0` succinct balance proof generation and verification.
- Wallet snapshot parsing and sequencer `getProofsAndRoot` acquisition.
- Fresh challenge, Ed25519 presenter binding, deterministic denials, replay
  cache locking, and negative verifier tests.
- Actual SPEL gate guest, account constraints, nonce rotation, access badge,
  recursive receipt assumption, generated IDL, and conditional execution tests.
- Official LEZ PPE composition code with real-mode guard and phase timing.
- Canonical LEZ deployment/public/private transaction packaging, restricted
  gate and badge signer files, stale-state rejection, and sequencer
  `getAccount`/`sendTransaction` command paths.
- Receipt-sized Messaging chunk/reassembly tests and official `logoscore`
  `chat_module` adapter.
- Sender- and challenge-bound GroupV2 admission workflow.
- Basecamp universal `ui_qml` source for token, gate, prove, verify, Messaging,
  and on-chain operations.

## External Validation Required Before Submission

- Complete a fresh recursive proof through the implemented `claim-submit`
  path and confirm acceptance by a running local LEZ sequencer before its
  10-minute timestamp window expires. Commodity CPU timing requires an
  accelerated prover in this environment.
- Run `demos/token-gated-chat/run.sh` with two online `chat_module` instances and
  confirm the live GroupV2 membership commit.
- Build/install the Basecamp `.lgx` with Nix, Qt, and
  `logos-module-builder`; capture desktop and narrow-window screenshots.
- Record final proof, PPE, network, and sequencer compute/cost measurements.
- Record the narrated demo video.
- Receive owner testing/approval.
- Open the separate Lambda Prize solution PR. Per project-owner instruction,
  no Lambda Prize PR has been created.

## Evidence Commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test -p proofgate-messaging
cargo test -p lez-gate-sdk
RISC0_DEV_MODE=0 cargo test -p attestation-prover \
  real_succinct_proof_round_trip -- --ignored --nocapture
```

See `docs/BENCHMARKS.md` for recorded results and explicit gaps.

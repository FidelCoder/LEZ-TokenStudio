# Implementation Status

## Implemented And Locally Validated

- Token select/create/mint configuration and wallet command adapters.
- Deterministic, asset-specific gate configuration.
- Exact LEZ account commitment, token data, and Merkle semantics.
- Risc0 guest checks for membership, token, threshold, and context.
- Real `RISC0_DEV_MODE=0` succinct balance proof generation and verification.
- Wallet snapshot parsing and current/`v0.2.0` sequencer membership-proof
  acquisition.
- Independent sequencer-root acquisition, VerificationChallenge v2 root
  binding, fresh challenges, Ed25519 presenter binding, deterministic denials,
  exact retained-challenge matching, replay cache locking, and malicious
  prover-selected-root/challenge rejection tests.
- Actual SPEL gate guest, account constraints, nonce rotation, access badge,
  recursive receipt assumption, generated IDL, and conditional execution tests.
- Official LEZ PPE composition code with real-mode guard and phase timing.
- Canonical LEZ deployment/public/private transaction packaging, restricted
  gate and badge signer files, stale-state rejection, and sequencer
  `getAccount`/`sendTransaction` command paths.
- Exact LEZ `v0.2.0` standalone node build and health check, live legacy
  membership-proof RPC, root-bound gate deployment, included GateState v2
  initialization, and authoritative root/state fetch/decode.
- Reproducible `scripts/ci-standalone-lez.sh` lifecycle plus a GitHub Actions
  job that builds the pinned node and exercises membership, deployment,
  inclusion, and root-bound initialization.
- Rejection of an uninitialized account by authoritative `fetch-badge`.
- Receipt-sized Messaging chunk/reassembly tests and official `logoscore`
  `chat_module` adapter.
- Sender- and challenge-bound GroupV2 admission workflow.
- Official two-daemon Chat doctest with 20 passing steps and a live ProofGate
  run covering encrypted challenge/proof delivery, real receipt verification,
  GroupV2 roster admission, forwarding denial, and replay denial for the prior
  challenge revision.
- Basecamp universal `ui_qml` source for token, gate, prove, verify, Messaging,
  and on-chain operations, including verifier-owned sequencer-root controls.
- A prior UI revision completed reproducible LGX packaging and four official
  Logos Qt integration tests, including a non-empty desktop render.
- Local CI entry point covering shell syntax, JSON, generated IDL equality,
  format, strict Clippy, and all workspace tests.

## External Validation Required Before Submission

- Complete a fresh recursive proof through `claim-submit` and confirm claim
  inclusion, badge creation, state rotation, and replay rejection before the
  10-minute timestamp window expires. Commodity CPU timing requires an
  accelerated prover in this environment.
- Install the built Basecamp `.lgx` in the full client and capture desktop and
  narrow-window screenshots. First rebuild and rerun the official integration
  suite for the current trusted-root control revision.
- Resolve the GitHub account-level billing lock so the added Actions jobs can
  start. Local CI is green evidence but does not replace LP-0005's green
  default-branch CI requirement.
- Obtain a current LEZ devnet/testnet endpoint, deployment access, and supported
  compute/cost metric; none is configured in this environment.
- Rerun the two-daemon Chat admission flow with VerificationChallenge v2 and
  exact retained-challenge enforcement.
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
cargo build --release -p proofgate
scripts/ci-local.sh
scripts/ci-standalone-lez.sh \
  /path/to/lez-v0.2.0/sequencer_service \
  /path/to/lez-v0.2.0/sequencer_config.json
cargo test -p proofgate-messaging
cargo test -p lez-gate-sdk
RISC0_DEV_MODE=0 cargo test -p attestation-prover \
  real_succinct_proof_round_trip -- --ignored --nocapture
```

See `docs/BENCHMARKS.md` for recorded results and explicit gaps.
The live runtime transcripts and package hashes are recorded in
`docs/evidence/LIVE_LOGOS_MESSAGING.md` and
`docs/evidence/BASECAMP_INTEGRATION.md`.

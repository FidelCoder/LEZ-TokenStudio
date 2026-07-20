# Contributing

## Prerequisites

- Rust and Cargo (the current development toolchain is Rust 1.91.0)
- Risc0 3.0.5 tooling and the Risc0 guest Rust component
- `jq` for demo scripts
- Nix with flakes for Logos Chat and Basecamp module integration

The workspace pins Risc0 exactly and pins Logos Execution Zone to `v0.2.0`.
Do not update either independently: guest image IDs and recursive receipt
compatibility must move together.

## Development Checks

Run before pushing:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
git diff --check
```

The real proof test is ignored because it takes minutes and consumes several
gigabytes of memory:

```bash
RISC0_DEV_MODE=0 cargo test -p attestation-prover \
  real_succinct_proof_round_trip -- --ignored --nocapture
```

The full recursive LEZ path is exercised by:

```bash
RISC0_DEV_MODE=0 cargo run -p proofgate -- on-chain compose <arguments>
```

Never use a development-mode receipt as submission evidence. The CLI rejects
`RISC0_DEV_MODE=1`, `true`, or `on` for proof generation and composition.

## Change Rules

- Keep guest inputs private and journals minimal.
- Add a negative test for every new verifier or gate rejection rule.
- Treat domain separators, canonical encodings, image IDs, account layouts,
  and error numbers as versioned protocol surfaces.
- Regenerate `programs/balance-gate/idl/balance_gate.json` after changing the
  SPEL guest interface.
- Do not commit presenter keys, wallet snapshots, witness files containing real
  accounts, replay caches, proof artifacts, or Basecamp runtime state.
- Do not claim a live sequencer, Messaging network, or `.lgx` validation unless
  that exact path was run and recorded.

## Repository Flow

Implementation changes are made and reviewed in this repository. A Lambda
Prize solution PR is a separate final action and must contain only the required
`solutions/LP-0005.md` file in the official prize repository.

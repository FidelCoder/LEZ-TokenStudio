# LEZ TokenStudio ProofGate

No-code token setup and private balance-gated access for Logos Execution Zone.

ProofGate targets **LP-0005: Private Token Balance Attestation**, a **$1,200
Logos Lambda Prize**. A holder proves that one hidden LEZ fungible-token account
has at least an exact public threshold without revealing the account ID, exact
balance, nonce, account data, Merkle path, nullifier key, or wallet history.

The product keeps token creation in the issuer workflow. Token creation is setup
UX; the bounty contribution is the reusable private attestation primitive and
its on-chain and off-chain consumers.

## What Is Implemented

- Token select, wallet-backed create/mint adapters, and deterministic gate JSON.
- Exact LEZ `v0.2.0` private-account commitment, token data, and Merkle proofs.
- Risc0 3.0.5 guest proving membership, token identity, and
  `hidden_balance >= threshold`.
- Fixture and live wallet/sequencer proof input through `getProofsAndRoot`.
- Real succinct proving guarded against `RISC0_DEV_MODE` and verified on output.
- Fresh Ed25519 presenter challenges, exact policy checks, deterministic errors,
  cross-process replay locking, and forwarding rejection.
- A real SPEL 0.6 `balance_gate` program that recursively verifies the balance
  receipt, rotates its nonce, and creates a time-bounded access badge.
- Composition through the official LEZ privacy preserving execution guest,
  canonical message packaging, badge-account signing, and sequencer RPC
  submission.
- Chunked, integrity-checked proof transport over official Logos
  `chat_module` calls and sender-bound GroupV2 admission.
- A Basecamp universal `ui_qml` module covering token, gate, prove, verify,
  Messaging, and on-chain flows.

## Privacy Boundary

Public proof claims are limited to the gate context hash, token program owner,
token definition, threshold, commitment root, presenter public key, issue time,
and optional expiry. The Risc0 journal never contains the private account ID or
exact balance. Reusing a presenter key can still create a linkable pseudonym;
use a separate key per context when unlinkability matters.

See [Privacy Model](docs/PRIVACY_MODEL.md) and [Security Policy](SECURITY.md).

## Repository Layout

```text
apps/basecamp-tokenstudio/       Basecamp QML module and asynchronous C++ bridge
crates/attestation-circuit/      Private statement evaluated by host and guest
crates/attestation-image-id/     Independently pinned balance guest image ID
crates/attestation-prover/       Wallet/RPC input acquisition and Risc0 prover
crates/attestation-types/        Versioned journals, envelopes, and challenges
crates/attestation-verifier/     Local verification and presenter binding
crates/balance-gate-core/        SPEL wire/state types and on-chain policy
crates/lez-compat/               Exact LEZ commitment and Merkle compatibility
crates/lez-gate-sdk/             Gate receipt and official PPE composition
crates/proofgate-messaging/      Logos Chat transfer and admission adapter
crates/tokenstudio-config/       Token/gate config and wallet command adapters
guests/balance-attestation/      Risc0 balance proof guest
programs/balance-gate/           SPEL gate guest and generated IDL
demos/token-gated-chat/          Real two-instance encrypted admission demo
scripts/demo.sh                  End-to-end real-mode demo
```

## Build And Test

```bash
cargo build --release -p proofgate
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Risc0 tooling 3.0.5 and its guest Rust component are required. The first build
downloads/compiles a substantial zkVM dependency graph.

Run the complete deterministic workflow:

```bash
scripts/demo.sh
```

This defaults to real balance proving and real recursive LEZ composition with
`RISC0_DEV_MODE=0`; it can take well over an hour on a small CPU. Use
`RUN_COMPOSITION=0 scripts/demo.sh` for the shorter proof plus actual SPEL
execution path. Set `ACCOUNT_SNAPSHOT` and `SEQUENCER_URL` to replace the
deterministic witness with live wallet/sequencer input.

The encrypted network phase requires two initialized Chat instances and is
enabled with `RUN_MESSAGING=1` plus the variables documented in
[the chat demo](demos/token-gated-chat/README.md).

## Core CLI Flow

Create or select a token and configure a gate:

```bash
proofgate token create <token fields> --wallet-binary wallet --output token.json
proofgate token select <token fields> --output token.json
proofgate token mint --token token.json --holder <account> --amount 100
proofgate gate init --token token.json --application-id tokenstudio \
  --gate-id founders --threshold 100 --verifier-id logos-chat:founders \
  --output gate.json
```

Generate and present a private proof:

```bash
proofgate presenter generate --output presenter.json
RISC0_DEV_MODE=0 proofgate prove --gate gate.json --input witness.json \
  --presenter-public-key-hex <64-hex> --output proof.json
proofgate challenge create --gate gate.json --output challenge.json
proofgate present --proof proof.json --challenge challenge.json \
  --presenter-key presenter.json --transport local --output envelope.json
proofgate verify --gate gate.json --envelope envelope.json \
  --replay-cache replay-cache.json
```

Use wallet/sequencer state instead of a fixture:

```bash
proofgate wallet snapshot --wallet-binary wallet \
  --account-id <private-account> --output snapshot.json
RISC0_DEV_MODE=0 proofgate prove --gate gate.json \
  --account-snapshot snapshot.json --sequencer-url http://127.0.0.1:8080 \
  --presenter-public-key-hex <64-hex> --output proof.json
```

## Consumers

The on-chain path deploys the embedded program, initializes gate state, creates
a badge-account-bound claim, executes the actual SPEL guest, recursively
composes an official LEZ PPE proof, signs the transaction, and submits it to a
configured sequencer. See [On-Chain Gate](docs/ON_CHAIN_GATE.md).

The off-chain path splits receipt-sized envelopes into bounded Chat messages,
verifies them locally, binds one sender address to one GroupV2 ID, and invokes
`add_group_member` only after `ALLOW`. See
[Logos Messaging](docs/LOGOS_MESSAGING.md).

The Basecamp module source and `.lgx` build instructions are in
[apps/basecamp-tokenstudio](apps/basecamp-tokenstudio/README.md).

## Validation Status

A real succinct balance receipt has been generated and verified locally. The
SPEL guest executes in LEZ's real guest executor, deterministic failures are
covered, the current IDL is committed, and the Messaging protocol passes a
receipt-sized 220 KB fixture.

The signed sequencer submission code is implemented and locally unit tested;
acceptance by a live sequencer, live two-node Messaging, and `.lgx` packaging
require external runtimes that are not installed in this workspace. They are
explicitly tracked in [Implementation Status](docs/STATUS.md), not represented
as completed evidence. Benchmark results and limits are in
[Benchmarks](docs/BENCHMARKS.md).

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Circuit Design](docs/CIRCUIT_DESIGN.md)
- [Prover Inputs](docs/PROVER_INPUTS.md)
- [Risc0 Proving](docs/RISC0_PROVING.md)
- [On-Chain Gate](docs/ON_CHAIN_GATE.md)
- [Logos Messaging](docs/LOGOS_MESSAGING.md)
- [Error Codes](docs/ERROR_CODES.md)
- [Token Setup](docs/TOKEN_SETUP.md)
- [LEZ Compatibility](docs/LEZ_COMPATIBILITY.md)
- [LP-0005 Execution Plan](docs/LP-0005_EXECUTION_PLAN.md)
- [Submission Playbook](docs/LAMBDA_PRIZE_SUBMISSION.md)

## Submission Policy

Implementation work is pushed only to
`FidelCoder/LEZ-TokenStudio:solution/lp-0005-proofgate`. No pull request to the
Lambda Prize repository will be created until the owner has tested and approved
the finished implementation.

Licensed under MIT OR Apache-2.0.

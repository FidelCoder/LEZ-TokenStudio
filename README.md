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
- Fixture and live wallet/sequencer proof input through `getProofsAndRoot`,
  with `v0.2.0` `getProofForCommitment` compatibility.
- Real succinct proving guarded against `RISC0_DEV_MODE` and verified on output.
- Fresh Ed25519 presenter challenges, exact policy checks, deterministic errors,
  independently pinned sequencer roots, cross-process replay locking, and
  forwarding rejection.
- A real SPEL 0.6 `balance_gate` program that recursively verifies the balance
  receipt, rotates its nonce, and creates a time-bounded access badge.
- Composition through the official LEZ privacy-preserving execution guest,
  canonical private transaction packaging, encrypted badge output, commitment
  and nullifier creation, private-account authorization, and sequencer RPC
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

A Merkle proof is only meaningful when its public root comes from a sequencer
the verifier trusts. Off-chain challenges therefore commit to a root obtained
independently by the verifier, and on-chain GateState stores the operator's
authorized root. A root supplied only by the proof holder is never trusted.

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
scripts/ci-local.sh
```

Run the complete Actions-independent submission validation against an exact
local LEZ v0.2.0 sequencer with:

```bash
scripts/ci-submission-local.sh \
  /path/to/sequencer_service \
  /path/to/sequencer_config.json
```

See [Offline Submission Validation](docs/OFFLINE_VALIDATION.md) for the evidence
layout and trust boundaries.

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
COMMITMENT_ROOT=$(proofgate sequencer-root \
  --sequencer-url http://127.0.0.1:3040)
proofgate challenge create --gate gate.json \
  --commitment-root-hex "$COMMITMENT_ROOT" --output challenge.json
proofgate present --proof proof.json --challenge challenge.json \
  --presenter-key presenter.json --transport local --output envelope.json
proofgate verify --gate gate.json --envelope envelope.json \
  --challenge challenge.json \
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

The on-chain path deploys the embedded program, initializes GateState v3,
creates a private-badge-bound claim, executes the actual SPEL guest, recursively
composes an official LEZ PPE proof, encrypts the badge into a commitment and
initialization nullifier, and submits the canonical private transaction to a
configured sequencer. See [On-Chain Gate](docs/ON_CHAIN_GATE.md).

The off-chain path splits receipt-sized envelopes into bounded Chat messages,
verifies them locally, binds one sender address to one GroupV2 ID, and invokes
`add_group_member` only after `ALLOW`. See
[Logos Messaging](docs/LOGOS_MESSAGING.md).

The Basecamp module source and `.lgx` build instructions are in
[apps/basecamp-tokenstudio](apps/basecamp-tokenstudio/README.md).

The proposal-bound governance consumer is in
[crates/proofgate-governance](crates/proofgate-governance) with a complete
walkthrough in [demos/private-governance](demos/private-governance/README.md).

## Validation Status

The complete owned technical path is validated locally without GitHub Actions.
A real `RISC0_DEV_MODE=0` private claim was included by an exact LEZ v0.2
sequencer, produced one public gate update plus one encrypted private badge
state, rejected stale replay, and persisted across restart. Current root-bound
Logos Chat evidence covers encrypted proof transport, sender-bound GroupV2
admission, forwarding denial, and replay denial. The Basecamp package passed all
four official Qt integration tests.

The deterministic `balance_gate` program is also deployed on the official
`https://testnet.lez.logos.co` sequencer:

- program ID
  `a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa`;
- deployment transaction
  `7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1`;
- initialized GateState v3 account
  `ee8068a772e5b928adfe4dbcc4752bf4ca6e514444b84222d8d8591d6c27c71b`;
- initialization transaction
  `f31b9c08215d2ec2d0cfd199db2713ca64ac781ad7640897f139b99db4d6f047`.

A separate proof-root-bound GateState completed the full real private claim:

- gate account
  `4f7b5ec8898c8509b4b18f39c646a838d37b6c00e7ffe251c410b91645640eb0`;
- initialization transaction
  `5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978`;
- private claim transaction
  `8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de`;
- fetched final state: GateState v3, counter `1`, nonce rotated, stale replay
  denied.

The proposal-bound governance crate supplies another distinct reference
consumer with persistent issued-challenge, replay, and pseudonymous-vote state.

The repository is not yet submission-ready under the official LP-0005 rubric.
CU/gas evidence for every on-chain operation, three testnet applications
including one built by an outside party, green CI on the public default branch,
and the narrated video are mandatory remaining outcomes. Local CI and cycle
counts are useful evidence but do not replace those explicit criteria. See
[Implementation Status](docs/STATUS.md), [Public Testnet
Evidence](docs/evidence/LIVE_LEZ_TESTNET.md), and
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
- [Local LEZ v0.2.0 Evidence](docs/evidence/LOCAL_LEZ_V0_2_0.md)
- [Live Logos Messaging Evidence](docs/evidence/LIVE_LOGOS_MESSAGING.md)
- [Official Basecamp Integration Evidence](docs/evidence/BASECAMP_INTEGRATION.md)
- [Live LEZ Testnet Evidence](docs/evidence/LIVE_LEZ_TESTNET.md)
- [Offline Submission Validation](docs/OFFLINE_VALIDATION.md)
- [External Integrator Guide](docs/EXTERNAL_INTEGRATOR_GUIDE.md)
- [LP-0005 Execution Plan](docs/LP-0005_EXECUTION_PLAN.md)
- [Submission Playbook](docs/LAMBDA_PRIZE_SUBMISSION.md)

## Submission Policy

The implementation repository is `FidelCoder/LEZ-TokenStudio` and the target
default branch is `main`. The completed local changes still require owner
review, commit, and push. No pull request to the Lambda Prize repository will
be created until the owner has tested and approved the finished implementation.

Licensed under MIT OR Apache-2.0.

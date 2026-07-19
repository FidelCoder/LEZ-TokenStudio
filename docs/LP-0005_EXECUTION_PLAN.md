# LP-0005 Execution Plan

## Positioning

The project should be submitted as:

**Solution: LP-0005 — TokenStudio ProofGate**

The idea is adjusted from "no-code token creation" to "no-code private
token-gating." The UI can still help create or select demo tokens, but the
deliverable that wins the $1,200 prize is a private token balance attestation
system that meets LP-0005.

## Product Flow

### Issuer Flow

1. Open TokenStudio ProofGate in Basecamp.
2. Select a token definition or create a demo token for the walkthrough.
3. Configure a gate:
   - token/program owner,
   - threshold `N`,
   - gate context id,
   - verifier public identity,
   - optional expiry.
4. Publish or share the gate config.

### Holder Flow

1. Select a shielded account in the LEZ wallet.
2. Generate a proof that the account's committed balance is at least `N`.
3. Review what the proof reveals:
   - threshold,
   - token/program owner,
   - gate context,
   - commitment root,
   - presenter public key.
4. Present proof either on-chain or over Logos Messaging.

### Verifier Flow

1. Receive a proof envelope.
2. Verify the Risc0 receipt and public journal.
3. Verify presenter binding with a fresh challenge signature.
4. Admit the user to a group or allow the gated on-chain action.

## Architecture

### `attestation-types`

Shared canonical structures:

- `GateContext`: application id, gate id, token/program owner, threshold,
  verifier id, expiry.
- `AttestationJournal`: public output of the circuit.
- `AttestationEnvelope`: Risc0 receipt, journal, challenge signature, metadata.
- deterministic serialization for hashing and signing.

### `balance-attestation` Risc0 Guest

Private witness:

- private account id,
- account fields: program owner, balance, nonce, account data hash,
- Merkle membership proof,
- presenter secret material or proof of knowledge input.

Public journal:

- threshold `N`,
- token/program owner,
- gate context hash,
- commitment root,
- presenter public key,
- circuit image id/version.

Checks:

- recompute LEZ private account commitment exactly as LEZ does,
- verify Merkle path reaches the public commitment root,
- prove `balance >= threshold`,
- bind proof to the gate context,
- bind proof to presenter identity to reduce proof forwarding.

### `attestation-prover`

Client-side library and CLI:

1. Read private account state from the wallet/local account store or wallet FFI.
2. Compute the account commitment.
3. Call sequencer RPC `getProofsAndRoot` with the commitment.
4. Build the Risc0 witness.
5. Generate the receipt with `RISC0_DEV_MODE=0`.
6. Emit an `AttestationEnvelope`.

### `balance-gate` LEZ Program

Reference on-chain consumer:

- accepts the attestation envelope or proof payload,
- verifies proof/journal,
- checks context, threshold, token/program owner, expiry, and presenter binding,
- allows a demo action such as `claim_access_badge` or `register_vote_power`.

The program must return deterministic documented errors for invalid proof,
wrong threshold, expired context, wrong presenter, and wrong token/program owner.

### `attestation-verifier`

Off-chain verifier library:

- validates the Risc0 receipt locally,
- validates the journal against the expected gate context,
- verifies a fresh challenge signature from the presenter,
- returns an allow/deny result without requiring an on-chain transaction.

### `token-gated-chat` Demo

Uses Logos Chat/Messaging:

1. Gate owner creates a group conversation.
2. Holder sends the proof envelope to the gate owner over chat.
3. Gate owner verifies locally.
4. Gate owner calls `add_group_member` for accepted holders.

The chat proof payload should be compact and signed. The demo should also show a
failed proof case and a forwarded-proof rejection case.

### Basecamp UI

Use the existing Logos `ui_qml` pattern:

- `metadata.json`,
- QML views,
- C++ backend,
- dependency on the LEZ wallet/module surface,
- dependency on chat/messaging for the off-chain path.

The UI should support three tabs:

- **Gate:** create/select token and configure threshold gate.
- **Prove:** pick shielded account and generate proof.
- **Verify:** verify local proof, submit on-chain proof, or admit to chat group.

## Success Criteria Mapping

- Shielded holder can generate threshold proof: `attestation-prover` + Risc0
  guest.
- No exact balance/account/nullifier leak: only journal fields are public.
- Context binding: `GateContext` hash included in the journal.
- Presenter binding: journal includes presenter key and verifier requires a
  fresh signature/challenge.
- Existing LEZ commitment format: mirror `Commitment::new` from
  `lee/state_machine/core/src/commitment.rs`.
- On-chain path: `programs/balance-gate`.
- Off-chain path: `demos/token-gated-chat` via Logos Chat/Messaging.
- Standalone consumer integration: chat gate plus on-chain access badge demo.
- Module/SDK: shared Rust crates and CLI.
- Basecamp GUI: QML module under `apps/basecamp-tokenstudio`.
- SPEL IDL: generated for `balance-gate`.
- Reliability: documented errors and failure tests.
- Performance: proof generation time and LEZ compute/cost report.
- Supportability: CI, README, local sequencer demo script, docs, and narrated
  video.

## Build Milestones

### M1: Skeleton and Types

- Rust workspace.
- Shared proof envelope types.
- Gate context hashing.
- CLI stubs.
- CI running formatting and unit tests.

### M2: Circuit Prototype

- Risc0 guest verifies LEZ commitment and Merkle path.
- Range check for `balance >= threshold`.
- Journal structure finalized.
- Unit tests for valid/invalid threshold and invalid path.

### M3: Prover and Off-Chain Verifier

- Client computes commitment and calls `getProofsAndRoot`.
- `RISC0_DEV_MODE=0` proof generation works locally.
- Off-chain verifier checks receipt, context, and presenter signature.
- Forwarded-proof rejection demo.

### M4: LEZ Verifier Program

- `balance-gate` program verifies proof for a demo gated action.
- Deterministic errors.
- SPEL IDL generated.
- Integration tests against local sequencer.

### M5: Logos Messaging Demo

- Token-gated chat flow.
- Holder sends proof envelope over Logos Chat/Messaging.
- Gate owner verifies and admits the holder.
- Failure cases included.

### M6: Basecamp UI and Final Submission

- QML module package.
- `scripts/demo.sh`.
- CU/proof benchmarks.
- Documentation.
- Narrated demo video.
- `solutions/LP-0005.md` PR to `logos-co/lambda-prize`.

## Main Risks

- On-chain proof verification cost may be high. Mitigation: benchmark early and
  keep the verifier action minimal.
- Logos module APIs are moving. Mitigation: build against current public wallet,
  chat, and module-builder patterns.
- Proof forwarding is explicitly called out by LP-0005. Mitigation: implement
  fresh challenge signature binding, and demonstrate a failed forwarded proof.
- `RISC0_DEV_MODE=0` can make demos slow. Mitigation: keep the witness small,
  cache setup artefacts, and print clear progress in `scripts/demo.sh`.

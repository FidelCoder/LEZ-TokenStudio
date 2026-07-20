# Solution: LP-0005 - TokenStudio ProofGate

**Submitted by:** FidelCoder

> Draft only. Do not submit until every unchecked item below has evidence and
> the project owner approves the final result.

## Summary

TokenStudio ProofGate is a reusable private LEZ token-balance attestation
primitive with two consumers. A Risc0 guest proves that a committed private
fungible-token account belongs to the current LEZ commitment tree and has
`balance >= threshold` without revealing the account ID, exact balance,
nonce, account data, nullifier key, or Merkle path.

The off-chain consumer transports a challenge and chunked presentation over
Logos Chat, verifies locally, and admits the authenticated sender to one
GroupV2 conversation. The on-chain consumer is a SPEL program that recursively
verifies the same balance receipt, rotates gate replay state, creates a
badge-account-bound access record, composes the official LEZ private-execution
proof, signs the canonical transaction, and submits it to a sequencer.

## Repository

- **Repo:** <https://github.com/FidelCoder/LEZ-TokenStudio>
- **Implementation branch:** `solution/lp-0005-proofgate`
- **License:** MIT OR Apache-2.0

## Approach

The balance statement mirrors LEZ `v0.2.0` commitment and Merkle semantics
instead of introducing a parallel token representation. Private witness fields
are consumed by the guest; only token policy, threshold, root, context,
presenter key, and time policy enter the public journal.

Proof forwarding is handled in layers. The receipt commits a presenter
Ed25519 key. Off-chain presentations sign a fresh verifier challenge and use a
cross-process locked replay cache. Chat admission additionally binds the group
and member address into the challenge nonce and requires the decrypted message
sender to equal that member. On-chain claims sign a domain-separated challenge
containing the program, gate, badge account, rotating nonce, and counter.

The SPEL gate uses `env::verify` for recursive receipt verification. A custom
composer is necessary because the stock LEZ wallet proving helper does not
accept the external balance-receipt assumption. After proving, ProofGate uses
the standard LEZ message, nonces, BIP-340 account signature, transaction
variant, and `sendTransaction` RPC contract.

Logos is material to the design: LEZ provides private account commitments and
trustless gate execution, while Logos Chat provides encrypted peer-to-peer
challenge and proof delivery. A hosted verifier API would learn request
metadata, become an admission authority, and create a censorship and
availability dependency.

## Success Criteria Checklist

- [x] Client-side proof checks a shielded fungible-token balance against public
  threshold `N`.
- [x] Receipt verification does not reveal account identity, exact balance,
  nullifier key, nonce, data, or Merkle path.
- [x] Proof journal is bound to an asset-specific gate context.
- [x] Presenter-key signatures prevent use by a recipient who only copies the
  proof.
- [x] Circuit and tests target the exact LEZ `v0.2.0` commitment format.
- [x] SPEL verifier recursively checks the receipt and gates access-badge
  creation.
- [x] Logos Messaging adapter exchanges challenges and bounded proof chunks,
  verifies locally, and conditionally calls `add_group_member`.
- [x] Standalone CLI/Basecamp consumer source and reproducible demo scripts are
  included.
- [x] Rust SDK, CLI, generated SPEL IDL, architecture, privacy, security, error,
  integration, and benchmark documentation are included.
- [x] Deterministic negative tests cover threshold, membership, token, context,
  challenge, presenter, replay, and transport-integrity failures.
- [x] Real `RISC0_DEV_MODE=0` balance proof generation and verification have
  been measured.
- [ ] Publish a green CI run from the final public/default branch.
- [ ] Build and attach the Basecamp `.lgx`; load it in Basecamp and capture
  desktop/narrow-window evidence.
- [ ] Run the encrypted two-instance Chat demo and record GroupV2 membership
  confirmation.
- [ ] Run `claim-submit` against a standalone LEZ sequencer with an
  accelerated prover and record the accepted transaction.
- [ ] Deploy the verifier to LEZ devnet/testnet and add the verified program ID.
- [ ] Record initialization/claim compute-unit or transaction-cost results.
- [ ] Record and link the narrated end-to-end video showing both paths and
  `RISC0_DEV_MODE=0`.

## FURPS Self-Assessment

### Functionality

ProofGate supports deterministic token/gate configuration, fixture or live
wallet/sequencer witness acquisition, real receipt generation, local
verification, encrypted Chat admission, SPEL simulation, recursive LEZ
composition, and signed sequencer submission. The token creation UI is setup
support; the reusable private attestation is the LP-0005 deliverable.

### Usability

The `proofgate` CLI gives each trust boundary an explicit command and writes
private keys with mode `0600`. The Basecamp module exposes token, gate, prove,
verify, Messaging, and sequencer operations through one asynchronous UI with
streamed output, cancellation, and persisted binary configuration.

### Reliability

All untrusted proof and message fields are bounded and validated before
cryptographic verification. Replay state is atomically replaced under an
exclusive lock. On-chain submission refetches account state and rejects a gate
that changed after claim signing. Stable denial codes are documented.

### Performance

On an Intel i5-7300U without prover acceleration, a warm real balance proof
took 206.65 seconds and produced a 223,970-byte receipt. The JSON proof was
299,244 bytes and its Logos Chat envelope used ten 32-KiB chunks plus one
manifest. Recursive composition did not complete reliably on this machine;
accepted sequencer CU/cost and accelerated composition timings remain required.

### Supportability

The workspace is split into versioned types, compatibility, circuit, prover,
verifier, gate, Messaging, configuration, and CLI crates. Local evidence
includes strict formatting, strict Clippy, 67 passing unit tests, generated-IDL
equality, shell syntax checks, and explicit external-validation tracking.

## Supporting Materials

- [Architecture](ARCHITECTURE.md)
- [Circuit design](CIRCUIT_DESIGN.md)
- [On-chain integration](ON_CHAIN_GATE.md)
- [Logos Messaging integration](LOGOS_MESSAGING.md)
- [Privacy model](PRIVACY_MODEL.md)
- [Benchmarks](BENCHMARKS.md)
- [Implementation status](STATUS.md)
- **Narrated demo:** TODO
- **Verified testnet program ID:** TODO
- **CI run:** TODO

## Terms & Conditions

Before submission, confirm agreement with the
[Lambda Prize Terms & Conditions](https://github.com/logos-co/lambda-prize/blob/master/TERMS.md).

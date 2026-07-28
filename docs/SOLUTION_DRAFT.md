# Solution: LP-0005 - TokenStudio ProofGate

**Submitted by:** FidelCoder

> Draft only. Add the narrated video and obtain project-owner approval before
> opening the Lambda Prize solution PR.

## Summary

TokenStudio ProofGate is a reusable private LEZ token-balance attestation
primitive with on-chain and off-chain consumers. A Risc0 guest proves that a
committed private fungible-token account belongs to a verifier-authorized LEZ
commitment tree and has `balance >= threshold` without revealing the account
ID, exact balance, nonce, account data, nullifier key, or Merkle path.

The off-chain consumer transports a retained root-bound challenge and a
chunked presentation over encrypted Logos Chat, verifies locally, and admits
the authenticated sender to one GroupV2 conversation. The on-chain consumer is
a SPEL program that recursively verifies the same receipt, rotates gate replay
state, and creates an encrypted holder-private access badge through the
official LEZ private execution path.

A third team-authored governance reference integration issues proposal-bound
challenges, verifies the real presentation through the reusable off-chain
verifier, persists replay state, and records one vote per presenter pseudonym.

## Repository

- **Repo:** <https://github.com/FidelCoder/LEZ-TokenStudio>
- **Default implementation branch:** `main`
- **License:** MIT OR Apache-2.0

## Approach

The balance statement mirrors exact LEZ `v0.2.0` commitment and Merkle
semantics. Private witness fields are consumed by the guest; only token policy,
threshold, root, context, presenter key, and time policy enter the public
journal.

The holder never chooses the trusted root alone. Off-chain, the verifier
independently authorizes a root, commits it into VerificationChallenge v2,
retains the challenge, and requires the presentation to match it exactly.
On-chain, the operator persists the authorized root in GateState v3. Both paths
reject a valid proof for any other root with code `1013`.

The receipt commits a presenter Ed25519 key. Off-chain presentations sign a
fresh verifier challenge and use a locked persistent replay cache. Chat
admission also binds the sender and GroupV2 identifier. On-chain claims bind
the program, gate, private badge identity, rotating nonce, and counter.

The access badge is not a public plaintext account. Its identity is derived
from holder-local ML-KEM private material. The canonical private transaction
contains one public gate update, one encrypted private badge state, one
commitment, and one initialization nullifier. Private-account authorization is
proved inside the LEZ PPE path; the transaction does not expose a public badge
signature.

The SPEL gate uses `env::verify` for recursive receipt verification. ProofGate
then uses the official LEZ privacy-preserving execution guest, canonical
private transaction type, and `sendTransaction` RPC contract. Logos Chat is
material to the second consumer: challenge and proof delivery stay encrypted
and admission remains local rather than delegated to a hosted verifier API.

## Success Criteria Checklist

- [x] Client-side proof checks a shielded fungible-token balance against public
  threshold `N`.
- [x] The proof hides account identity, exact balance, nullifier key, nonce,
  account data, and Merkle path.
- [x] Authorized-root, context, presenter, fresh-challenge, and replay binding
  are enforced on both paths.
- [x] The circuit and tests target the LEZ private commitment semantics.
- [x] The SPEL verifier issues an encrypted private badge through the canonical
  LEZ private transaction shape.
- [x] A real `RISC0_DEV_MODE=0` claim completed against an exact local LEZ
  sequencer with inclusion, rotation, replay denial, and restart persistence.
- [x] Current encrypted Logos Chat admission completed with forwarding and
  replay denial.
- [x] The Basecamp LGX passed all four official Qt integration tests.
- [x] A proposal-bound private-governance reference consumer is implemented and
  tested.
- [x] The verifier program and a real private claim are included on the
  official public LEZ testnet with a verified program ID, counter rotation,
  private badge binding, and stale replay denial.
- [x] Architecture, privacy, security, errors, integration, benchmark, and
  reproducibility documentation are included.
- [ ] Document supported devnet/testnet CU or gas cost for every on-chain
  operation.
- [ ] Provide three distinct testnet integrations with at least one application
  built by a party outside the submitting team.
- [ ] Make CI green on the public default branch.
- [ ] Record and link the mandatory narrated end-to-end video.

## FURPS Self-Assessment

### Functionality

ProofGate covers deterministic token/gate configuration, fixture or
wallet/sequencer witness acquisition, real receipt generation, local
verification, encrypted Chat admission, SPEL execution, recursive LEZ
composition, private transaction submission, and holder-local badge recovery.
The token setup UI supports the reusable LP-0005 attestation and its two
consumers.

### Usability

The `proofgate` CLI gives each trust boundary an explicit command and stores
presenter, gate, and private badge keys with mode `0600`. The Basecamp module
exposes token, gate, prove, verify, Messaging, and sequencer workflows through
one asynchronous UI with output streaming and cancellation.

### Reliability

Untrusted proof and message fields are bounded before expensive processing.
Replay state is atomically replaced under a lock. On-chain submission refetches
gate state and rejects changes after claim composition. The SDK enforces
exactly one public state, one encrypted private state, one commitment, and one
nullifier. Current tests cover threshold, membership, token, context, root,
challenge, presenter, replay, transport integrity, private output shape, and
persistence.

### Performance

On an Intel i5-7300U without prover acceleration, a warm balance proof took
206.65 seconds and produced a 223,970-byte receipt. The complete recursive claim
took 5,718,392 ms: 3,527,334 ms for the gate proof and 2,191,056 ms for the
outer PPE proof. The gate used 3,670,016 total guest cycles and the outer guest
used 1,048,576. The resulting LEZ proof was 230,611 bytes.

The public-testnet private claim took 5,783,248 ms: 3,866,660 ms for the gate
proof and 1,916,587 ms for the outer PPE proof. It produced the same 223,970-byte
balance receipt and 230,611-byte PPE proof, used 3,670,016 gate cycles and
1,048,576 outer cycles, and was included as transaction
`8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de`.

The current 299,830-byte Chat presentation crossed the encrypted channel as ten
chunks plus one manifest. Network propagation latency was not instrumented.
The public testnet included deployment, initialization, and the private claim,
but its transaction RPC exposes no CU/gas field. Guest cycles are reported
without mislabeling them as network transaction cost; the mandatory CU
criterion remains open.

### Supportability

The final local program ID is
`a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa`.
Deployment transaction
`7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1`,
initialization transaction
`056e5a2354c60c2bd69f5288ccf88855c2882abc0c443f63638af6c0067d6945`,
and private claim transaction
`d92dc10b87e7b33891a76c80b2796cfdf736b6fd25b749e8a5de2282477d512b`
were included and remained queryable after restart.

The workspace is split into versioned types, LEZ compatibility, circuit,
prover, verifier, gate, Messaging, configuration, CLI, SPEL program, and
Basecamp module boundaries. Local runners export logs, public artifacts,
environment details, and SHA-256 manifests without secret keys or witnesses.

## Supporting Materials

- [Architecture](ARCHITECTURE.md)
- [Circuit design](CIRCUIT_DESIGN.md)
- [On-chain integration](ON_CHAIN_GATE.md)
- [Logos Messaging integration](LOGOS_MESSAGING.md)
- [Privacy model](PRIVACY_MODEL.md)
- [Benchmarks](BENCHMARKS.md)
- [Implementation status](STATUS.md)
- [Local LEZ v0.2.0 evidence](evidence/LOCAL_LEZ_V0_2_0.md)
- [Live Logos Messaging evidence](evidence/LIVE_LOGOS_MESSAGING.md)
- [Official Basecamp integration evidence](evidence/BASECAMP_INTEGRATION.md)
- [Live LEZ testnet evidence](evidence/LIVE_LEZ_TESTNET.md)
- [External integrator guide](EXTERNAL_INTEGRATOR_GUIDE.md)
- [Offline local validation](OFFLINE_VALIDATION.md)
- **Narrated demo:** TODO
- **Verified local program ID:**
  `a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa`
- **Local validation:** `artifacts/local-submission-final`
- **Verified public testnet program ID:**
  `a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa`

## Terms & Conditions

Before submission, confirm agreement with the
[Lambda Prize Terms & Conditions](https://github.com/logos-co/lambda-prize/blob/master/TERMS.md).

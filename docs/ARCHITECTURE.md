# Architecture

## Components

`tokenstudio-config` owns deterministic token and gate configuration. Token
creation and mint commands are adapters over the official LEZ wallet CLI; gate
configuration is consumed unchanged by all proof paths.

`lez-compat` mirrors LEZ `v0.2.0` private account commitments, fungible token
account data, and Merkle membership semantics. Compatibility vectors pin field
order and little-endian encoding.

`attestation-circuit` evaluates the private statement shared by host tests and
the Risc0 guest. `balance-attestation-methods` builds the guest ELF and
generated image ID. `attestation-image-id` stores the independently checked
image ID so the guest does not depend on its own generated artifact.

`attestation-prover` reads fixture or wallet snapshots, obtains a sequencer
membership proof through current `getProofsAndRoot` or the pinned `v0.2.0`
`getProofForCommitment` fallback, builds the witness, creates a real succinct
receipt, and verifies it before returning.

`attestation-verifier` verifies the receipt and exact gate policy, requires
the journal root to equal the verifier's independently trusted sequencer root,
then checks a domain-separated Ed25519 presentation over a fresh verifier
challenge. Replay state is owned by the CLI workflow and persisted atomically
under an exclusive process lock.

`balance-gate-core` defines SPEL-compatible instruction/account types,
challenge binding, deterministic errors, nonce rotation, and access badges.
`programs/balance-gate` is the actual SPEL guest. It recursively verifies the
balance receipt and emits a time-bounded LEZ state transition.

`lez-gate-sdk` supplies the balance-gate receipt as an assumption to the
official LEZ privacy-preserving execution guest. It constructs an ML-KEM-backed
private badge identity, encrypts the AccessBadge post-state, and returns
sequencer-format proof bytes with one public state, one private commitment, and
one initialization nullifier.

`proofgate-messaging` chunks presentations for the official `chat_module`,
checks transfer integrity, receives by polling the same API used in Logos
doctests, and performs sender-bound GroupV2 admission.

`proofgate` is the common CLI used by scripts and the Basecamp module. The
Basecamp C++ bridge launches only this binary with structured `QProcess`
arguments; cryptographic policy is not reimplemented in QML or C++.

## Off-Chain Flow

1. Verifier reads the current root from its trusted sequencer endpoint.
2. Verifier creates a random, expiring challenge for one gate and identity,
   committing the trusted root into the challenge digest.
3. Holder proves its private account satisfies the gate.
4. Holder signs the challenge, image ID, receipt digest, and canonical journal.
5. Presentation is delivered locally or over encrypted Logos Chat.
6. Verifier requires the envelope challenge to equal the exact challenge it
   retained, then checks receipt, journal, trusted root, gate, time, signature,
   and replay state before returning `ALLOW`.

For chat admission, the challenge identity additionally commits to the group
and member address, and the received Chat sender must equal that member.

## On-Chain Flow

1. Gate operator stores a trusted root with immutable gate policy, rotating
   nonce, bounded proof age, and claim counter in public GateState v3.
2. Holder creates a restricted private badge identity and signs a claim bound
   to its derived account ID, the program, gate, nonce, and counter.
3. SPEL constraints authorize the mutable public gate and the new private badge
   output; gate logic checks policy and the presenter signature.
4. `env::verify` resolves the balance-attestation receipt assumption.
5. The program updates GateState and creates the AccessBadge plaintext with an
   explicit timestamp validity window.
6. The official LEZ PPE guest recursively consumes the program receipt,
   encrypts the private badge, and proves authorization from its nullifier
   secret key.
7. The client requires exactly one public post-state, one encrypted private
   post-state, one commitment, and one initialization nullifier.
8. It packages the public gate ID and nonce plus those private outputs into the
   canonical LEZ private transaction and calls `sendTransaction`.
9. After inclusion, authoritative GateState shows the incremented counter and
   rotated nonce; replaying the old claim is rejected before reproving.

## Versioned Boundaries

- Gate/token JSON schema: version 1
- Attestation journal: version 2
- Verification challenge: version 2
- On-chain challenge: version 1
- Gate state: version 3
- Access badge: version 1
- Messaging transfer: version 1
- Risc0: 3.0.5
- SPEL: 0.6.0 commit `0cb7e0980535af619482cf1c823f4d394b3ebd61`
- Logos Execution Zone: `v0.2.0`

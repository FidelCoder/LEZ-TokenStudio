# Architecture

## Components

`tokenstudio-config` owns deterministic token and gate configuration. Token
creation and mint commands are adapters over the official LEZ wallet CLI; gate
configuration is consumed unchanged by all proof paths.

`lez-compat` mirrors LEZ `v0.2.0` private account commitments, fungible token
account data, and Merkle membership semantics. Compatibility vectors pin field
order and little-endian encoding.

`attestation-circuit` evaluates the private statement shared by host tests and
the Risc0 guest. `balance-attestation-methods` builds the guest ELF and generated
image ID. `attestation-image-id` stores the independently checked image ID so
the guest does not depend on its own generated artifact.

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
official LEZ privacy preserving execution guest and returns sequencer-format
proof bytes, public post states, commitments, and nullifiers.

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

1. Gate operator reads a root from its trusted sequencer and stores it with the
   immutable gate policy, rotating nonce, and claim counter.
2. Holder signs a claim bound to program, gate, badge account, nonce, and
   counter.
3. SPEL account constraints authorize the mutable gate and new badge accounts.
4. Gate logic checks exact public claims and presenter signature.
5. `env::verify` resolves the balance-attestation receipt assumption.
6. Program updates gate state, creates `AccessBadge`, and applies a timestamp
   validity window.
7. The official LEZ PPE guest recursively consumes the program receipt and
   creates the final private-execution proof.
8. The client packages public account IDs, current nonces, post-states, and the
   proof into the canonical LEZ message.
9. The new badge account signs that message and the client submits the
   privacy-preserving transaction through `sendTransaction`.

## Versioned Boundaries

- Gate/token JSON schema: version 1
- Attestation journal: version 2
- Verification challenge: version 2
- On-chain challenge: version 1
- Gate state: version 2
- Access badge: version 1
- Messaging transfer: version 1
- Risc0: 3.0.5
- SPEL: 0.6.0 commit `0cb7e0980535af619482cf1c823f4d394b3ebd61`
- Logos Execution Zone: `v0.2.0`

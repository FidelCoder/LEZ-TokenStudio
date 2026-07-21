# Changelog

## 0.1.0 - Unreleased

- Added deterministic token selection, wallet-backed token creation/mint
  adapters, and asset-specific gate configuration.
- Added exact LEZ private-account commitment and Merkle membership semantics.
- Added a Risc0 balance-attestation guest and real succinct host proving.
- Added wallet snapshot and sequencer `getProofsAndRoot` input acquisition,
  including a `getProofForCommitment` fallback for the pinned LEZ `v0.2.0`
  RPC.
- Added challenge-bound local verification, deterministic denials, persistent
  replay protection, and restricted presenter key files.
- Added the SPEL `balance_gate` program, generated IDL, access badges, nonce
  rotation, timestamp windows, and recursive official LEZ PPE composition.
- Added restricted LEZ signer generation, canonical transaction packaging,
  stale-state checks, program deployment, gate initialization, and signed
  sequencer claim submission.
- Added bounded Logos Messaging transfer, local receive/verify, sender-bound
  GroupV2 admission, encrypted challenge exchange, forwarding/replay demos,
  and protocol documentation.
- Added a Basecamp `ui_qml` module for token, gate, prove, verify, Messaging,
  and on-chain workflows.
- Made the Basecamp proof workflow populate presenter keys automatically,
  default to the bundled witness path, use the verifier's full five-minute
  challenge window, and reject missing proof inputs before starting a
  presentation.
- Corrected SPEL initialization and badge creation ownership so LEZ applies the
  authorized program owner after guest execution.
- Added authoritative access-badge fetching and decoding by sequencer RPC.

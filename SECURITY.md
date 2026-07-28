# Security Policy

## Status

ProofGate is research and bounty implementation code. It has not received a
professional security audit and must not be used to protect production funds,
governance, or access without an independent review.

Report vulnerabilities through GitHub's private security advisory flow for
`FidelCoder/LEZ-TokenStudio`. Do not include presenter keys, wallet snapshots,
private badge keys, or private account data in a public issue.

## Protected Properties

- The Risc0 guest proves an exact LEZ commitment opening, Merkle membership,
  token identity, and `balance >= threshold` without journaling private account
  fields or exact balance.
- Off-chain presentations are bound to a fresh verifier challenge and the
  presenter key committed in the receipt.
- Replay caches are atomically persisted while holding an exclusive
  cross-process lock.
- On-chain claims bind program, gate context, private badge account, nonce,
  counter, image ID, and journal; successful claims rotate the nonce.
- Claim submission refetches public gate state, rejects stale state, validates
  that the restricted private badge key derives the claimed account ID, and
  requires exactly one public update plus one encrypted private output,
  commitment, and initialization nullifier.
- Chat admission binds the target GroupV2 ID and member address into the signed
  challenge and requires the encrypted message sender to match that address.

## Assumptions

- Risc0 receipt verification, SHA-256, Ed25519, Borsh, SPEL, ML-KEM, and LEZ
  account semantics behave as specified by the pinned versions.
- The sequencer supplies an authentic commitment root and membership proof.
- The wallet snapshot and presenter key are read from a trusted local machine.
- Gate, presenter, and private badge key files remain mode `0600` and are
  protected from local disclosure.
- The verifier's clock is accurate enough for challenge and proof freshness.
- Logos Chat correctly authenticates the `sender` associated with a decrypted
  message.
- Gate operators protect replay-cache files and do not run separate verifiers
  against separate caches for the same verifier identity.

## Known Limitations

- A reused presenter public key can link presentations even though the private
  account and exact balance remain hidden. Use a separate key per privacy
  context when unlinkability matters.
- The public journal reveals the token identifiers, threshold, commitment root,
  context hash, presenter public key, issue time, and optional expiry.
- GateState v3 stores a bounded proof-age policy from one minute through 24
  hours. The default is ten minutes; the unaccelerated local recursive demo
  uses six hours. Longer windows increase exposure to older authorized roots.
- The holder's access-badge plaintext is a sensitive local artifact. The
  sequencer receives only its ciphertext, commitment, and nullifier; inclusion
  binds that ciphertext but does not provide a plaintext fetch API.
- The CLI adapter trusts the configured `proofgate`, `wallet`, and
  `logoscore` executable paths. The Basecamp backend invokes ProofGate without
  a shell but does not sandbox the binary itself.
- Group admission is asynchronous. If `add_group_member` fails after the replay
  cache commits, the operator must issue a new challenge and retry.
- Denial-of-service, side-channel resistance, compromised hosts, malicious
  binaries, and consensus/sequencer faults are outside this demo's guarantees.

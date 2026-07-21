# Balance Attestation Circuit

## Statement

The private witness contains a full LEZ private account and its commitment
membership path. The circuit also receives the claimed commitment root, a
public gate context, a presenter public key, and an issuance timestamp.

The circuit proves all of the following:

1. Recomputing the LEZ v0.3 account commitment from the private account fields
   yields a commitment included in the public commitment root.
2. The account is owned by the configured LEZ token program.
3. The committed account data is the Borsh encoding of
   `TokenHolding::Fungible`.
4. The holding's token definition account matches the gate's public token
   definition ID.
5. The private fungible token balance is at least the public threshold.
6. The presenter public key is nonzero and is committed into the journal.
7. The issuance time is not later than the configured gate expiry.

The circuit proves membership under the root recorded in its journal. Root
authority is deliberately enforced outside the balance circuit: an off-chain
VerificationChallenge v2 or on-chain GateState v2 must contain a root obtained
independently by the verifier. Accepting only the root carried by a holder's
proof would let that holder construct an unrelated private tree.

## Public Journal

The journal reveals only:

- schema version,
- gate context hash,
- token program ID,
- token definition account ID,
- threshold,
- commitment root,
- presenter public key,
- issuance time,
- optional expiry.

It does not reveal the private account ID, account nonce, account data, exact
fungible balance, or the account's native LEZ balance.

## Why Token Data Is Parsed

LEZ uses one token program for many fungible token definitions. The account
`program_owner` therefore identifies the token program but not the asset.
The specific token definition and fungible balance are encoded in the committed
account data. ProofGate validates both. This prevents a holder from satisfying a
gate with a different asset managed by the same token program.

## Presenter Binding

The circuit commits a presenter public key into the journal. Verification later
requires a signature from that key over a fresh challenge containing the gate
ID, verifier ID, and nonce. A copied proof is therefore unusable without the
presenter private key. Challenge freshness and replay storage are verifier
responsibilities.

## Versioning

The context domain is `LEZ-TokenStudio/GateContext/v2` and the journal schema
version is 2. Any statement or LEZ commitment layout change requires a new
context domain, journal version, and Risc0 image ID.

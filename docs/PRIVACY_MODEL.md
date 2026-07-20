# Privacy Model

## Private Inputs

The following values stay inside the holder's witness and are not written to
the Risc0 journal:

- private LEZ account ID;
- exact fungible token balance;
- native account balance;
- account nonce;
- complete account data;
- Merkle sibling path and leaf position;
- wallet storage and broader transaction history.

The receipt proves knowledge of values that recreate one commitment in the
public root and satisfy the configured statement.

## Public Claims

The journal intentionally reveals:

- gate context hash;
- token program owner and definition ID;
- threshold being proven;
- commitment root;
- presenter public key;
- proof issue timestamp; and
- optional gate expiry.

The proof reveals only that the hidden fungible balance is at least the exact
public threshold. It does not reveal how far above the threshold the balance
is.

## Transport Metadata

Off-chain envelopes also contain a random verifier challenge, verifier ID,
challenge timestamps, receipt bytes, presenter signature, and transport label.
Logos Chat encrypts message content, but normal network and chat metadata remain
subject to the privacy properties of Logos Messaging itself.

On-chain claims reveal the badge account being initialized, gate state update,
validity window, and access badge fields. LEZ private execution hides private
account identities according to its own circuit and transaction model.

## Linkability

The presenter public key is stable within one proof. Reusing it across gates or
presentations can create a linkable pseudonym. Holders seeking unlinkability
should create a new presenter key for each gate or access context.

The commitment root and token definition can also correlate proofs generated
against the same state snapshot and asset. ProofGate makes these public because
the verifier must know which state and token policy were proven.

## Non-Goals

ProofGate does not hide the fact that a holder requested access, make the public
threshold secret, anonymize network metadata beyond Logos Messaging, or prove
that a balance remains unspent forever. Fresh challenges and proof-age limits
bound replay and stale-state risk; they do not replace consensus finality.

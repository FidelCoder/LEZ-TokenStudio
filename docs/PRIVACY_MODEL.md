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
public root and satisfy the configured statement. It does not prove that an
arbitrary root chosen by the holder is a canonical LEZ root.

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

The on-chain transaction reveals the public gate account update, validity
window, encrypted private post-state, private commitment, and initialization
nullifier. The private badge account ID and AccessBadge plaintext are not placed
in the public post-state or canonical public-account list. The holder retains
the badge plaintext locally, while LEZ private execution protects it according
to the pinned PPE circuit and transaction model.

## Linkability

The presenter public key is stable within one proof. Reusing it across gates or
presentations can create a linkable pseudonym. Holders seeking unlinkability
should create a new presenter key for each gate or access context.

The commitment root and token definition can also correlate proofs generated
against the same state snapshot and asset. ProofGate makes these public because
the verifier must know which state and token policy were proven. Private
commitments and nullifiers have LEZ's normal transaction-level linkability
properties.

## Root Trust And Freshness

The verifier or gate operator chooses the sequencer endpoint it trusts. The
off-chain verifier reads a root from that endpoint and commits it into
VerificationChallenge v2. The on-chain operator stores an authorized root in
GateState v3. Both paths reject a valid receipt whose journal contains any
other root with deterministic code `1013`.

Off-chain verification also requires the verifier's retained challenge file.
This prevents a holder from creating and signing a replacement challenge that
contains a root chosen by that holder.

The `sequencer-root` command obtains this value without consuming a holder's
proof artifact. On LEZ `v0.2.0`, which has no direct root RPC, it asks the
selected sequencer for a membership proof of the protocol's default account
anchor and derives that response's root using LEZ's own Merkle semantics. This
trusts the selected sequencer endpoint; it does not convert one sequencer into
consensus finality.

An authorized root is a snapshot. A verifier should issue a fresh challenge
from its current trusted root. An on-chain gate currently requires a new
GateState when its operator wants to authorize a newer root; root rotation is
not an implemented instruction. GateState's bounded proof-age field limits how
long a receipt against that root can be claimed.

## Non-Goals

ProofGate does not hide the fact that a holder requested access, make the public
threshold secret, anonymize network metadata beyond Logos Messaging, or prove
that a balance remains unspent forever. Fresh challenges, explicitly authorized
roots, and proof-age limits bound replay and stale-state risk; they do not
replace consensus finality.

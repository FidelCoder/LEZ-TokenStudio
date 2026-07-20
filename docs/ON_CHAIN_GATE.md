# On-Chain Balance Gate

ProofGate's on-chain path is a real Logos Execution Zone (LEZ) program built
with SPEL 0.6. It consumes a private balance-attestation receipt as a recursive
Risc0 assumption and produces an ordinary LEZ private-execution proof.

## Trust Boundary

The balance-attestation guest proves all private facts:

- the holder knows a private account opening;
- the account commitment belongs to the supplied LEZ commitment root;
- the account belongs to the configured token program and token definition;
- the hidden balance is at least the exact gate threshold.

Its public journal contains only the gate context, token identifiers,
threshold, commitment root, presenter public key, issue time, and optional
expiry. It does not reveal the account ID, account opening, exact balance,
Merkle path, or wallet history.

The `balance_gate` SPEL program then enforces the public policy and possession
binding:

1. Load and validate the persisted `GateState`.
2. Match the journal's context, token, threshold, and expiry exactly.
3. Verify an Ed25519 signature by the presenter key committed in the receipt.
4. Recursively verify the balance-attestation receipt by its fixed Risc0 image
   ID.
5. Rotate the gate nonce and increment the claim counter.
6. Initialize a claim-account-specific `AccessBadge`.
7. Restrict the LEZ output timestamp to the proof freshness and gate expiry
   window.

The resulting program receipt is supplied to the official LEZ privacy
preserving execution (PPE) guest. The PPE output contains the public post
states, private commitments, and nullifiers expected by the LEZ sequencer.

## Replay And Forwarding Resistance

The holder signs a domain-separated on-chain challenge containing:

- the `balance_gate` program ID;
- the badge account ID;
- the gate context hash;
- the current 32-byte gate nonce;
- the current claim counter;
- the balance-attestation image ID and canonical journal.

Moving a signed claim to another program, gate, badge account, state version,
or counter invalidates the signature. A successful claim derives a new nonce
from the old nonce and signature, so the same claim cannot be replayed against
the next state.

## Time Policy

On-chain proofs are accepted from 30 seconds before their issue timestamp until
the earliest of:

- 10 minutes after issue; or
- the configured gate expiry.

LEZ timestamp validity uses an exclusive upper bound, so the program adds one
millisecond when representing an inclusive expiry. The 10-minute freshness
policy is deliberately fixed in the program today. Real local proving can take
longer on constrained hardware; such a proof remains cryptographically valid
but must be regenerated before submission to a current sequencer.

## Program Accounts

`Initialize` consumes one signer account and writes a program-owned
`GateState`. An expiry argument of `0` means no gate expiry.

`Claim` consumes:

- a mutable `GateState` account owned by the current program; and
- a new signer badge account that becomes a program-owned `AccessBadge`.

The generated SPEL IDL is committed at
`programs/balance-gate/idl/balance_gate.json`.

## Sequencer Flow

```bash
proofgate on-chain program-id

proofgate on-chain account-generate --output gate-account.json
proofgate on-chain account-id --key gate-account.json

proofgate on-chain deploy --sequencer-url http://127.0.0.1:3040

proofgate on-chain init \
  --gate examples/gates/founders.json \
  --challenge-nonce-hex <32-byte-hex> \
  --output gate-state.json

proofgate on-chain initialize-submit \
  --sequencer-url http://127.0.0.1:3040 \
  --state gate-state.json \
  --gate-key gate-account.json
```

After the initialization transaction is included, fetch authoritative state
from the sequencer. The gate account ID can be copied in hexadecimal form from
`account-generate` or `account-id`.

```bash
proofgate on-chain fetch-state \
  --sequencer-url http://127.0.0.1:3040 \
  --gate-account-id-hex <gate-account-id> \
  --output gate-state.json

proofgate on-chain account-generate --output badge-account.json
proofgate on-chain account-id --key badge-account.json

proofgate on-chain challenge \
  --state gate-state.json \
  --claim-account-id-hex <badge-account-id>

proofgate on-chain present \
  --proof proof.json \
  --state gate-state.json \
  --claim-account-id-hex <badge-account-id> \
  --presenter-key presenter.json \
  --output claim.json

RISC0_DEV_MODE=0 proofgate on-chain claim-submit \
  --sequencer-url http://127.0.0.1:3040 \
  --proof proof.json \
  --state gate-state.json \
  --claim claim.json \
  --gate-account-id-hex <gate-account-id> \
  --badge-key badge-account.json \
  --output-lez-proof lez-proof.bin
```

`claim-submit` refetches both public accounts before proving. It rejects a
gate state that changed after the claim was signed, rejects an initialized
badge account, recursively verifies the balance receipt, composes the official
LEZ PPE receipt, builds the canonical LEZ message, signs it with the
badge-account key, and calls `sendTransaction`. Signer files are created with
mode `0600` and are never accepted on the command line.

On-chain proofs have a ten-minute timestamp window. Use an accelerated prover
for sequencer submission if composition cannot complete inside that window on
the local CPU.

## Offline Execution

```bash
proofgate on-chain simulate \
  --state gate-state.json \
  --claim claim.json \
  --gate-account-id-hex <gate-account-id> \
  --badge-account-id-hex <badge-account-id>

RISC0_DEV_MODE=0 proofgate on-chain compose \
  --proof proof.json \
  --state gate-state.json \
  --claim claim.json \
  --gate-account-id-hex <gate-account-id> \
  --badge-account-id-hex <badge-account-id> \
  --output-lez-proof lez-proof.bin
```

`simulate` executes the actual SPEL guest with a conditional receipt claim. It
is useful for fast policy and account-transition testing but does not create a
cryptographic proof. `compose` requires real Risc0 proving and verifies both
receipts before writing the LEZ proof bytes. The offline commands accept
synthetic account IDs for deterministic testing; only `claim-submit` packages
fresh sequencer state into a signed transaction.

Run the complete node-backed demo branch with:

```bash
RUN_SEQUENCER=1 \
SEQUENCER_URL=http://127.0.0.1:3040 \
ACCOUNT_SNAPSHOT=/path/to/private-account.json \
TOKEN_OWNER_HEX=<token-program-id> \
TOKEN_DEFINITION_HEX=<token-definition-id> \
scripts/demo.sh
```

Set `DEPLOY_GATE=0` when the embedded program ID is already deployed.

## Pinned Dependencies

- SPEL: commit `0cb7e0980535af619482cf1c823f4d394b3ebd61`
  (`v0.6.0`)
- Logos Execution Zone: tag `v0.2.0`
- Risc0: `3.0.5`

The program ID and balance-attestation image ID are generated from the guest
ELFs. Tests fail if the shared balance image ID drifts from the generated
method ID.

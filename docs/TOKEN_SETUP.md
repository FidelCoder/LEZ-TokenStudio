# Token And Gate Setup

ProofGate keeps token creation as the issuer onboarding flow. The private
attestation remains the security boundary and the LP-0005 deliverable.

## Select An Existing Token

Create a reusable token config without submitting a transaction:

```bash
cargo run -p proofgate -- token select \
  --name "Founders Token" \
  --symbol FNDR \
  --definition-account-id-hex <32-byte-token-definition-id> \
  --token-program-owner-hex <32-byte-token-program-id> \
  --definition-account Private/<definition-account-id> \
  --supply-account Private/<supply-account-id> \
  --issuer-account Private/<issuer-account-id> \
  --total-supply 1000000 \
  --output token.json
```

## Create A LEZ Token

The create command invokes the official LEZ wallet command
`wallet token new`. Use `--dry-run` to inspect the exact invocation without
submitting a transaction.

```bash
cargo run -p proofgate -- token create \
  --wallet-binary /path/to/wallet \
  --name "Founders Token" \
  --symbol FNDR \
  --definition-account-id-hex <32-byte-token-definition-id> \
  --token-program-owner-hex <32-byte-token-program-id> \
  --definition-account Private/<definition-account-id> \
  --supply-account Private/<supply-account-id> \
  --issuer-account Private/<issuer-account-id> \
  --total-supply 1000000 \
  --output token.json
```

The LEZ wallet requires `LEE_WALLET_HOME_DIR` and an initialized local wallet.
ProofGate inherits the current process environment and never reads wallet seed
material.

## Mint To A Holder

```bash
cargo run -p proofgate -- token mint \
  --wallet-binary /path/to/wallet \
  --token token.json \
  --holder Private/<holder-account-id> \
  --amount 250
```

## Create A Private Gate

```bash
cargo run -p proofgate -- gate init \
  --token token.json \
  --application-id tokenstudio \
  --gate-id founders-chat \
  --threshold 100 \
  --verifier-id logos-chat:founders \
  --output gate.json
```

The gate context binds both the LEZ token program ID and the specific token
definition account ID. This matters because all fungible token types share the
same LEZ token program; checking only the program owner could accept a balance
from the wrong asset.

# LEZ TokenStudio ProofGate

No-code private token-gating for Logos Execution Zone.

This project is scoped for **LP-0005: Private Token Balance Attestation**, a
Large Logos Lambda Prize with a **$1,200** prize. The product keeps the
TokenStudio angle, but the winning core is not plain token creation. The
winning core is a reusable private balance attestation primitive: a holder can
prove they own at least `N` tokens without revealing their account, exact
balance, nullifier public key, or wider wallet history.

## Target Prize

- **Prize:** LP-0005 Private Token Balance Attestation
- **Prize amount:** $1,200
- **Submission route:** Pull request to `logos-co/lambda-prize`
- **Implementation repo:** `https://github.com/FidelCoder/LEZ-TokenStudio`

## Adjusted Idea

TokenStudio becomes **ProofGate mode**:

1. Issuer creates or selects a LEZ token for a gated community, governance
   action, fee tier, or allowlist.
2. Issuer configures a gate: token program owner, threshold, context id, expiry,
   and verifier identity.
3. Holder generates a client-side proof that their shielded token balance is at
   least the configured threshold.
4. The proof is usable in two paths:
   - On-chain: a LEZ verifier program gates a protected action.
   - Off-chain: the proof is sent over Logos Chat/Messaging and verified locally
     before admitting the holder to a group.

Token creation remains a useful demo/onboarding workflow, but it is deliberately
secondary. LP-0013 covered token authorities and was a $600 prize; LP-0005 is
the correct $1,200 target because it centers private token balance proofs.

## Why This Has Impact

Most token-gated flows leak too much: wallet address, token identity, balance,
and historical activity. On Logos, balances are private by default, so access
control needs a private alternative. ProofGate gives apps a reusable primitive
for:

- private token-gated chat rooms,
- private governance participation thresholds,
- private protocol fee tiers,
- private holder-only dashboards,
- private allowlist eligibility checks.

The user gets a simple interface, while the system still exercises the hard
Logos primitives LP-0005 asks for: LEZ private accounts, Merkle membership
proofs, Risc0 proof generation, on-chain verification, off-chain verification,
and Logos Messaging.

## Proposed Repository Shape

```text
LEZ-TokenStudio/
├── apps/
│   └── basecamp-tokenstudio/      # QML + C++ Basecamp UI module
├── crates/
│   ├── attestation-types/         # shared proof envelope and journal types
│   ├── attestation-prover/        # client-side proof generator
│   ├── attestation-verifier/      # off-chain local verifier
│   └── attestation-cli/           # issuer/holder/verifier CLI
├── guests/
│   └── balance-attestation/       # Risc0 guest proving balance >= threshold
├── programs/
│   └── balance-gate/              # LEZ verifier program and gated action demo
├── demos/
│   └── token-gated-chat/          # Logos Chat/Messaging integration demo
├── scripts/
│   └── demo.sh                    # reproducible end-to-end demo
└── docs/
    ├── LP-0005_EXECUTION_PLAN.md
    └── LAMBDA_PRIZE_SUBMISSION.md
```

## Current Status

The repo has been cloned and is ready for implementation. The next build step is
to scaffold the Rust workspace, wire LEZ/Risc0 dependencies, and implement the
shared proof envelope before the circuit and integrations.

## Sources Checked

- Logos Lambda Prize: <https://github.com/logos-co/lambda-prize>
- LP-0005: <https://github.com/logos-co/lambda-prize/blob/master/prizes/LP-0005.md>
- Logos Execution Zone: <https://github.com/logos-blockchain/logos-execution-zone>
- LEZ Wallet UI: <https://github.com/logos-blockchain/logos-execution-zone-wallet-ui>
- Logos Chat Module: <https://github.com/logos-co/logos-chat-module>

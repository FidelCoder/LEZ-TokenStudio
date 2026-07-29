# Private governance integration

This reference application uses the reusable off-chain verifier to authorize one
pseudonymous vote per qualifying presenter. The gate context binds the proof to a
single proposal, while the service stores issued and consumed challenge digests
so a holder-selected or replayed challenge cannot be accepted.

Build the two CLIs:

```bash
cargo build --release -p proofgate -p proofgate-governance
```

Generate a proof for the proposal gate from a real private testnet account. The
snapshot is a private file and must not be committed:

```bash
export LEE_WALLET_HOME_DIR=/path/to/isolated-wallet
target/release/proofgate presenter generate --output /tmp/presenter.json
target/release/proofgate wallet snapshot \
  --wallet-binary /path/to/wallet \
  --account-id Private/ACCOUNT_ID \
  --output /tmp/account-snapshot.json
target/release/proofgate prove \
  --gate examples/gates/governance-treasury-7.json \
  --account-snapshot /tmp/account-snapshot.json \
  --sequencer-url https://testnet.lez.logos.co \
  --presenter-public-key-hex PRESENTER_PUBLIC_KEY_HEX \
  --output /tmp/governance-proof.json
```

The governance service issues the challenge. Use the current live commitment
root returned by `proofgate sequencer-root`:

```bash
target/release/proofgate-governance challenge \
  --gate examples/gates/governance-treasury-7.json \
  --proposal-id treasury-7 \
  --commitment-root-hex LIVE_ROOT \
  --state /tmp/treasury-7-state.json \
  --output /tmp/treasury-7-challenge.json
```

Present and cast:

```bash
target/release/proofgate present \
  --proof /tmp/governance-proof.json \
  --challenge /tmp/treasury-7-challenge.json \
  --presenter-key /tmp/presenter.json \
  --transport local \
  --output /tmp/treasury-7-presentation.json
target/release/proofgate-governance cast \
  --gate examples/gates/governance-treasury-7.json \
  --proposal-id treasury-7 \
  --presentation /tmp/treasury-7-presentation.json \
  --choice yes \
  --state /tmp/treasury-7-state.json
target/release/proofgate-governance status \
  --state /tmp/treasury-7-state.json
```

The state file is created with mode `0600`. It contains presenter pseudonyms,
not private account IDs or balances. A new presenter key creates a new pseudonym,
so one-person-one-vote is not claimed; the policy is one vote per proof-bound
presenter key for this proposal.

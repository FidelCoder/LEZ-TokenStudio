# Prover Inputs

## Deterministic Fixture

Generate the local demo witness and its single-leaf membership root:

```bash
cargo run -p attestation-prover --example generate_demo_input
```

The resulting `examples/witnesses/founders.json` contains private demo account
fields and is written with mode `0600` on Unix. It is synthetic and committed
only to make evaluator runs reproducible. Real wallet snapshots must never be
committed.

Generate a real succinct proof from the fixture:

```bash
RISC0_DEV_MODE=0 cargo run -p proofgate -- prove \
  --gate examples/gates/founders.json \
  --input examples/witnesses/founders.json \
  --presenter-public-key-hex 5555555555555555555555555555555555555555555555555555555555555555 \
  --issued-at-unix-ms 1800000000000 \
  --output /tmp/founders.proof.json
```

## Live Wallet And Sequencer

Capture the account from the official LEZ wallet. The command invokes
`wallet account get --raw --account-id ...`, parses the structured account
JSON, and writes a restricted local snapshot without printing private fields.

```bash
cargo run -p proofgate -- wallet snapshot \
  --wallet-binary /path/to/wallet \
  --account-id Private/<base58-account-id> \
  --output holder.snapshot.json
```

Request a commitment membership proof from a sequencer and prove the live
account. ProofGate first uses the current `getProofsAndRoot` RPC and
automatically falls back to LEZ `v0.2.0`'s `getProofForCommitment` RPC:

```bash
RISC0_DEV_MODE=0 cargo run -p proofgate -- prove \
  --gate gate.json \
  --account-snapshot holder.snapshot.json \
  --sequencer-url http://127.0.0.1:3040 \
  --presenter-public-key-hex <32-byte-hex-public-key> \
  --output holder.proof.json
```

Before proving, the host recomputes the LEZ commitment and rejects a sequencer
response whose membership path does not produce the returned root. The same
membership calculation runs again inside the Risc0 guest. The legacy RPC does
not return the root separately, so ProofGate derives it from the returned path
and commitment using the same LEZ `MembershipProof::compute_root` semantics.
An ignored integration test exercises this fallback against a real node.

## Independent Verifier Root

Proof input acquisition and verifier root selection are separate trust
operations. Before issuing an off-chain challenge or initializing an on-chain
gate, the verifier/operator reads a root from its own configured sequencer:

```bash
COMMITMENT_ROOT=$(cargo run -p proofgate -- sequencer-root \
  --sequencer-url http://127.0.0.1:3040)

cargo run -p proofgate -- challenge create \
  --gate gate.json \
  --commitment-root-hex "$COMMITMENT_ROOT" \
  --output challenge.json
```

Do not copy the trusted root from `proof.json` or accept it from the holder.
With `RUN_SEQUENCER=1`, `scripts/demo.sh` queries the configured sequencer
independently. Only its deterministic fixture mode derives the root from the
local proof to keep a self-contained demo reproducible; that shortcut is not a
production trust model.

## Sensitive Files

Wallet snapshots and prover input fixtures contain account IDs, exact balances,
nonces, and account data. Proof files do not contain those private fields. Keep
real snapshots outside source control and delete them when no longer needed.

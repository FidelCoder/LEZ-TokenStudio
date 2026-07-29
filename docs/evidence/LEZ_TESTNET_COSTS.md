# LEZ Testnet Transaction Cost Evidence

## Scope

The LP-0005 deployment, initialization, and private claim were re-fetched from
the official `https://testnet.lez.logos.co` sequencer on 2026-07-28. The public
evidence collector records inclusion, the serialized RPC result length, and a
SHA-256 digest of each complete `getTransaction` response before removing the
large raw response from the shareable bundle.

| Operation | Transaction | Included | Serialized result characters | Network CU/gas reported |
| --- | --- | ---: | ---: | --- |
| Deploy `balance_gate` | `7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1` | yes | 783,660 | not exposed |
| Initialize GateState v3 | `5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978` | yes | 1,032 | not exposed |
| Submit private claim | `8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de` | yes | 309,844 | not exposed |

The checksummed summaries are in
`artifacts/testnet-claim-current/rpc/{deployment,initialization,claim}.json`.
The final fetched GateState has `claim_counter = 1`, proving that the claim was
executed rather than merely accepted by a client.

## Measurement Boundary

The official sequencer's `getTransaction` result contains the serialized
transaction and block ID but no compute-unit, gas-used, fee, or equivalent
execution-cost field. The current official LEZ source defines a wallet-side
`GasConfig` data type, but the checked source has no call site for it and no RPC
that returns per-transaction cost metadata or official public-testnet gas
parameters.

ProofGate separately reports 3,670,016 Risc0 cycles for the gate guest and
1,048,576 cycles for the outer PPE guest. Those are cryptographic guest
measurements, not LEZ network CU, and are intentionally not relabeled as
transaction cost.

This is the complete cost evidence supported by the current public interface.
A numeric network-CU value requires an official sequencer measurement method,
published testnet gas parameters, or evaluator guidance.

## Reproduce

```bash
cargo build --release -p proofgate
scripts/ci-testnet-evidence.sh \
  artifacts/testnet-claim-current \
  4f7b5ec8898c8509b4b18f39c646a838d37b6c00e7ffe251c410b91645640eb0 \
  5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978 \
  8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de
```

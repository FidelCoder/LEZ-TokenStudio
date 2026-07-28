# Live LEZ Testnet Evidence

## Network

- Captured: 2026-07-28
- Sequencer: `https://testnet.lez.logos.co`
- Health: `checkHealth` returned success
- Evidence collector: `scripts/ci-testnet-evidence.sh`
- Checksummed full-claim bundle: `artifacts/testnet-claim-current`

The endpoint is the default public endpoint in the official LEZ wallet source:
<https://github.com/logos-blockchain/logos-execution-zone/blob/main/lez/wallet/src/config.rs>.

## Included Program Deployment

| Field | Value |
| --- | --- |
| Program ID | `a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa` |
| Deployment transaction | `7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1` |
| RPC inclusion | non-null `getTransaction` result |
| Deployed payload | deterministic embedded `balance_gate` ELF |

The release CLI independently matched the embedded program ID and fetched the
included deployment from the official endpoint.

## Live Private Token And Balance Proof

| Field | Value |
| --- | --- |
| Token definition account | `2267ee0f5f4fd0cc12788469f322f70226d797ecc79e222fad1cc5f74f9f5b8e` |
| Official token ProgramId | `c5d50f88bfe7cb14b421673e9441aade7571e522eef035cc24d80b2e53c69a7c` |
| Private-token transaction | `70740bfb7b701f41a7e5c99921114194ac865986e4860193f35e50a260bd0c0a` |
| Gate context hash | `7b31674500a7787efae5ea74bb84ee6307b4b298400f7d23327539011d806e05` |
| Threshold | `100` |
| Proof commitment root | `47be6686d338e627d8333e1d6210964aacd56b0d6a81192aaaea7ad1b7f508fa` |
| Balance receipt | 223,970 bytes |
| Cryptographic mode | `RISC0_DEV_MODE=0` |

The pinned exact LEZ v0.2 wallet created the test token with an encrypted private
supply account. ProofGate captured that account locally, requested its live
membership proof directly from the testnet, and produced the public receipt
without publishing the snapshot or exact private account state.

## Included Private Claim

| Field | Value |
| --- | --- |
| Gate account | `4f7b5ec8898c8509b4b18f39c646a838d37b6c00e7ffe251c410b91645640eb0` |
| Initialization transaction | `5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978` |
| Private badge account | `666e14ec27217227692569b0557b86c4edc6f4ffe8a6d82f7d668cd23310ee99` |
| Private claim transaction | `8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de` |
| Initial / final counter | `0 / 1` |
| Final state | GateState v3, nonce rotated |
| Stale replay | denied before recomposition |
| Maximum proof age | 21,600,000 ms |

The gate was initialized after proof generation with the exact receipt root.
The claim used the canonical LEZ private transaction shape: one public gate
post-state, one encrypted private badge post-state, one private commitment, and
one initialization nullifier. The fetched final state, badge context,
presenter, proof timestamp, and rotated nonce all matched independently.

## Live Recursive Metrics

| Measurement | Result |
| --- | ---: |
| Gate proving | 3,866,660 ms |
| Official PPE proving | 1,916,587 ms |
| Total composition | 5,783,248 ms |
| Gate total / user / paging cycles | 3,670,016 / 3,293,423 / 168,952 |
| Gate segments | 4 |
| PPE total / user / paging cycles | 1,048,576 / 779,323 / 85,755 |
| PPE segments | 1 |
| LEZ PPE proof | 230,611 bytes |

## Reproduce Inclusion Evidence

```bash
cargo build --release -p proofgate
scripts/ci-testnet-evidence.sh \
  artifacts/testnet-claim-current \
  4f7b5ec8898c8509b4b18f39c646a838d37b6c00e7ffe251c410b91645640eb0 \
  5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978 \
  8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de
```

The collector verifies deployment, initialization, and claim inclusion, then
requires GateState v3 with `claim_counter = 1`. It records response hashes and
lengths, removes raw base64 transaction payloads, and regenerates a SHA-256
manifest.

The public bundle includes the gate/token policy, balance receipt, claim, initial
and final public state, PPE proof, sanitized log, and RPC summaries. It excludes
wallet storage, recovery phrases, private account snapshots, presenter keys,
badge keys, and the holder-local access badge.

## Cost Boundary

The official sequencer RPC returns an opaque serialized transaction plus block
ID and exposes no CU/gas field. Current official LEZ source likewise has no
execution-cost metadata RPC. The measured Risc0 cycles above are retained as
cryptographic compute evidence and are not mislabeled as testnet CU. The
mandatory network-CU criterion still requires a supported measurement method
or evaluator guidance.

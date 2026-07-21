# Benchmarks

## Environment

- Date: 2026-07-19 to 2026-07-21
- OS: Ubuntu 24.04.4 LTS, x86_64
- CPU: Intel Core i5-7300U, 2 cores / 4 threads, up to 3.5 GHz
- Memory: 15 GiB RAM, 4 GiB swap
- Hardware prover acceleration: none configured
- Rust: 1.91.0
- Risc0: 3.0.5
- SPEL: 0.6.0 commit `0cb7e0980535af619482cf1c823f4d394b3ebd61`
- LEZ dependency: tag `v0.2.0`, commit `a58fbce2`
- Mode: `RISC0_DEV_MODE=0` for cryptographic proof measurements

This is deliberately modest development hardware. Results are not estimates
for a GPU, Bonsai, Boundless, or production prover.

## Balance Attestation

| Measurement | Result |
| --- | ---: |
| Warm real succinct proof and round-trip verification | 206.65 s |
| Real receipt bytes | 223,970 |
| JSON proof artifact | 299,244 bytes |
| Public journal exact balance/account leakage | none |

A later cold/contention run took approximately 45 minutes. The machine was
under substantial memory pressure; this is recorded as an operational warning,
not a representative steady-state benchmark.

## Gate Execution

| Measurement | Result |
| --- | ---: |
| Actual SPEL guest conditional execution test suite contribution | about 6.85 s |
| Valid transition | gate counter 0 to 1; access badge claim 1 |
| Invalid presenter | deterministic `1009` failure |

Conditional execution validates guest logic and account transitions but is not
a cryptographic receipt benchmark.

## Standalone LEZ v0.2.0

The final gate ELF was tested against an exact LEZ `v0.2.0` standalone
sequencer built from commit `a58fbce2` and listening on
`http://127.0.0.1:3040`.

| Evidence | Result |
| --- | --- |
| Health RPC | successful |
| Legacy membership RPC | `getProofForCommitment` fallback passed |
| Independently queried root | `d4e0961e5e2774178dad46069bfb7cd610b883417f7ce07475c9c61d3b45575f` |
| Root-bound gate program ID | `19936a0b1174095ae7c5978d1ee547741b81c01d6432e167c02a8a84fcec9a0d` |
| Deployment transaction | `d09c590a68e3754908d4bdb87e8dfc2152cf15636ae011d3d23a65094e293a66` |
| Gate account | `514a3e3cd0f1ea199afddcf48e1bf90372439cccc4090c2e95d815f03f94cd79` |
| Initialization transaction | `8f73d4a79715a1069ea2c3e9ee66ff94030cf2d9bd57ee27856fd4f66aa8bc96` |
| Transaction indexing | both queryable by observed height 21 |
| Decoded GateState version/root | version 2 / exact queried root |
| Decoded gate context | `c8136d53893ad946bec8cbe7a4bdb4734618d7290fa2ef685f18bacc06f55c95` |
| Decoded claim counter | `0` |

These IDs belong to the root-bound ELF. Deployment, public initialization,
`getAccount`, and GateState v2 decoding are live-node evidence. The
reproducible `scripts/ci-standalone-lez.sh` run independently completed the
same lifecycle from a clean temporary node. Private claim inclusion and
compute/cost measurements are not claimed.
See the [full local sequencer record](evidence/LOCAL_LEZ_V0_2_0.md) for the
compatibility and ownership findings.

## Messaging

| Measurement | Result |
| --- | ---: |
| Fixture receipt payload | 220,000 bytes |
| Default decoded chunk size | 32,768 bytes |
| Maximum decoded chunk size | 49,152 bytes |
| Maximum transfer | 4 MiB / 128 chunks |
| Receipt-sized protocol tests | 7 passed |
| Official encrypted Chat doctest | 20 passed / 0 failed / 0 skipped |
| Live real receipt | 223,970 bytes |
| Live signed presentation | 299,730 bytes |
| Live transfer | 10 chunks + 1 manifest |
| Live admission result | GroupV2 member observed |

The real 299,730-byte presentation required ten default chunks plus one
manifest. It was reconstructed and verified locally before the holder appeared
in the verifier-owned GroupV2 roster. Forwarding and replay were rejected with
codes 1008 and 1010. The live run did not instrument Waku propagation time, so
no network latency measurement is claimed. This measurement predates
VerificationChallenge v2; a root-bound network rerun is not claimed. See the
[redacted runtime record](evidence/LIVE_LOGOS_MESSAGING.md).

## Recursive LEZ Composition

The CLI records separate gate-receipt, outer PPE, and total milliseconds. A
fully completed result must be added after the instrumented real run succeeds.
An earlier uninstrumented run was interrupted after more than one hour in the
gate proving phase and produced no artifact. A second instrumented run remained
in gate proving for approximately 90 minutes and exited without producing an
artifact or preserving final stderr after its detached session expired.

No sequencer compute-unit or transaction cost is claimed. The standalone node
accepted deployment and initialization, but final cost evidence must include a
private claim and come from the network's supported metric rather than host
execution time.

## Reproduction

```bash
RISC0_DEV_MODE=0 cargo test -p attestation-prover \
  real_succinct_proof_round_trip -- --ignored --nocapture

RISC0_DEV_MODE=0 target/release/proofgate on-chain compose \
  --proof proof.json \
  --state gate-state.json \
  --claim claim.json \
  --gate-account-id-hex <64-hex> \
  --badge-account-id-hex <64-hex> \
  --output-lez-proof lez-proof.bin

scripts/ci-standalone-lez.sh \
  /path/to/lez-v0.2.0/sequencer_service \
  /path/to/lez-v0.2.0/sequencer_config.json
```

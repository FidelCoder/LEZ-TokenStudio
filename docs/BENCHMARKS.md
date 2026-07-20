# Benchmarks

## Environment

- Date: 2026-07-19 to 2026-07-20
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

## Messaging

| Measurement | Result |
| --- | ---: |
| Fixture receipt payload | 220,000 bytes |
| Default decoded chunk size | 32,768 bytes |
| Maximum decoded chunk size | 49,152 bytes |
| Maximum transfer | 4 MiB / 128 chunks |
| Receipt-sized protocol tests | 7 passed |

The real 299,730-byte envelope requires ten default chunks plus one manifest.
Live Waku propagation time depends on the selected Logos fleet and remains an
external integration measurement.

## Recursive LEZ Composition

The CLI records separate gate-receipt, outer PPE, and total milliseconds. A
fully completed result must be added after the instrumented real run succeeds.
An earlier uninstrumented run was interrupted after more than one hour in the
gate proving phase and produced no artifact. A second instrumented run remained
in gate proving for approximately 90 minutes and exited without producing an
artifact or preserving final stderr after its detached session expired.

No sequencer compute-unit or transaction cost is claimed yet. The final value
must come from an accepted local/testnet transaction, not host execution time.

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
```

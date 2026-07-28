# Benchmarks

## Environment

- Date: 2026-07-19 through 2026-07-28
- OS: Ubuntu 24.04.4 LTS, x86_64
- CPU: Intel Core i5-7300U, 2 cores / 4 threads, up to 3.5 GHz
- Memory: 15 GiB RAM, 4 GiB swap
- Hardware prover acceleration: none configured
- Rust/Cargo: 1.91.0
- Risc0: 3.0.5
- SPEL: 0.6.0 commit `0cb7e0980535af619482cf1c823f4d394b3ebd61`
- LEZ node: tag `v0.2.0`, commit `a58fbce2`
- Cryptographic mode: `RISC0_DEV_MODE=0`

This is modest development hardware. The results are measurements, not
estimates for GPU, Bonsai, Boundless, or a production prover.

## Balance Attestation

| Measurement | Result |
| --- | ---: |
| Warm real succinct proof and round-trip verification | 206.65 s |
| Real receipt | 223,970 bytes |
| JSON proof artifact | 299,244 bytes |
| Public journal exact balance/account leakage | none |

A separate cold/contention run took about 45 minutes while the machine was
under heavy memory pressure. That is an operational warning rather than a
steady-state benchmark.

## Real Recursive LEZ Claim

A complete `RISC0_DEV_MODE=0` claim ran against the exact LEZ `v0.2.0`
sequencer and was included.

| Measurement | Result |
| --- | ---: |
| Gate proving | 3,527,334 ms |
| Outer PPE proving | 2,191,056 ms |
| Total composition | 5,718,392 ms (about 95 min 18 s) |
| Balance receipt | 223,970 bytes |
| LEZ PPE proof | 230,611 bytes |
| Gate total/user/paging cycles | 3,670,016 / 3,295,256 / 168,939 |
| Gate segments | 4 |
| Outer total/user/paging cycles | 1,048,576 / 779,413 / 85,755 |
| Outer segments | 1 |
| Public post-states | 1 |
| Encrypted private post-states | 1 |
| Private commitments | 1 |
| Initialization nullifiers | 1 |
| Gate transition | counter 0 to 1; nonce rotated |
| Private badge | claim 1 |
| Stale replay | denied |

The local fixture lifecycle used GateState v3
`max_proof_age_ms=21,600,000` (six hours) so an unaccelerated recursive proof
could complete. Normal gates retain the secure ten-minute default; accepted
configuration is bounded from one minute through 24 hours.

The original lifecycle wrapper returned status 1 only after the claim was
included because its final assertion compared the badge presenter byte array to
the proof presenter hex string. The assertion now normalizes both forms. Every
post-inclusion check passed against the preserved artifacts, and the exact node
was restarted on its preserved RocksDB state to confirm all three transactions
and the final GateState remained queryable.

## Exact Standalone LEZ v0.2.0

| Evidence | Result |
| --- | --- |
| Health RPC | successful |
| Legacy membership RPC | `getProofForCommitment` fallback passed |
| Independently queried default-anchor root | `d4e0961e5e2774178dad46069bfb7cd610b883417f7ce07475c9c61d3b45575f` |
| Fixture proof/gate root | `510dbd3aa09bae25bc9e683e65ff2104eba32565f9c10b91582596955505b778` |
| Program ID | `a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa` |
| Gate account | `9956ee9d8da1563b6f287dd6ecce79d2c901a57f09a532466c7f6a13d8fbc03c` |
| Private badge account ID | `4cb98cd381d1f8b95064843becf9aef06a4344fa085aec52023236f75274b679` |
| Deployment transaction | `7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1` |
| Initialization transaction | `056e5a2354c60c2bd69f5288ccf88855c2882abc0c443f63638af6c0067d6945` |
| Private claim transaction | `d92dc10b87e7b33891a76c80b2796cfdf736b6fd25b749e8a5de2282477d512b` |
| State after restart | GateState v3, counter 1, exact final state match |
| Transactions after restart | all 3 queryable |

The default-anchor root demonstrates live legacy RPC compatibility. The
deterministic claim intentionally initializes the gate with the fixture root
committed by the real proof; it does not substitute the unrelated default
anchor for the proof root.

## Public LEZ Testnet

| Evidence | Result |
| --- | --- |
| Endpoint | `https://testnet.lez.logos.co` |
| Health | successful |
| Program ID | `a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa` |
| Deployment transaction | `7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1` |
| Private-token transaction | `70740bfb7b701f41a7e5c99921114194ac865986e4860193f35e50a260bd0c0a` |
| Claim gate account | `4f7b5ec8898c8509b4b18f39c646a838d37b6c00e7ffe251c410b91645640eb0` |
| Initialization transaction | `5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978` |
| Private claim transaction | `8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de` |
| Fetched final state | GateState v3, counter 1, nonce rotated |
| Stale replay | denied before recomposition |
| Gate / PPE / total proving | 3,866,660 / 1,916,587 / 5,783,248 ms |
| Gate / PPE total cycles | 3,670,016 / 1,048,576 |
| Balance receipt / PPE proof | 223,970 / 230,611 bytes |
| RPC CU/gas field | not exposed |

Deployment, proof-root-bound initialization, and the private claim were
included and independently re-fetched by `scripts/ci-testnet-evidence.sh`.
See [Live LEZ Testnet
Evidence](evidence/LIVE_LEZ_TESTNET.md).

## Messaging

| Measurement | Result |
| --- | ---: |
| Default decoded chunk size | 32,768 bytes |
| Maximum decoded chunk size | 49,152 bytes |
| Maximum transfer | 4 MiB / 128 chunks |
| Official encrypted Chat doctest | 20 passed / 0 failed / 0 skipped |
| Current real receipt | 223,970 bytes |
| Current signed presentation | 299,830 bytes |
| Current transfer | 10 chunks + 1 manifest |
| Delivery copies | 11 unique messages x 2 |
| Admission | holder observed in GroupV2 |
| Forwarding / replay | denied with 1008 / 1010 |

The current run used VerificationChallenge v2, an independently authorized
root, and exact retained-challenge matching. It did not instrument Waku
propagation time, so no network latency is claimed.

## Basecamp

| Measurement | Result |
| --- | ---: |
| Current LGX | 765,585 bytes |
| Current LGX SHA-256 | `dc6938044ae38805bf8cbefde908a7467d4392e70a12cfd2d4bb529911ee5145` |
| Official Qt integration tests | 4 passed / 0 failed |
| Desktop render | 1024 x 768, 47,633 bytes |
| Render SHA-256 | `a3017630cf163b77c0b1c2e6e56669808124ab854f50cafbcabddaab355161ad` |

## Cost Boundary

The official testnet sequencer's `getTransaction` method returns a serialized
transaction and block ID, but no CU/gas field. The public deployment,
initialization, and private claim are therefore verified without claiming an
unsupported network cost. Local guest cycles and host proving time are recorded
exactly above, but they are not LEZ testnet CU.

LP-0005 still requires CU cost for each on-chain operation. That criterion needs
a supported LEZ measurement method or explicit evaluator guidance; it is not
satisfied by relabeling Risc0 cycles.

## Reproduction

```bash
scripts/ci-local.sh
RISC0_DEV_MODE=0 cargo test -p attestation-prover \
  real_succinct_proof_round_trip -- --ignored --nocapture
scripts/ci-standalone-claim.sh \
  /path/to/lez-v0.2.0/sequencer_service \
  /path/to/lez-v0.2.0/sequencer_config.json
scripts/basecamp-local.sh artifacts/basecamp-current
```

The preserved public claim record is in
`artifacts/local-submission-final/standalone-claim`. See
[Local LEZ Evidence](evidence/LOCAL_LEZ_V0_2_0.md), [Live Messaging
Evidence](evidence/LIVE_LOGOS_MESSAGING.md), and [Basecamp
Evidence](evidence/BASECAMP_INTEGRATION.md).

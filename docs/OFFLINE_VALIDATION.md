# Offline Submission Validation

ProofGate can run its complete technical validation locally without GitHub
Actions. The local runner executes the same Rust formatting, strict Clippy,
workspace-test, generated-IDL, and metadata checks, then adds a release build,
an exact LEZ v0.2.0 node lifecycle, a real `RISC0_DEV_MODE=0` recursive fixture
claim with inclusion/badge/rotation/replay assertions, a reproducible Basecamp
package, and the official Basecamp integration output.

## Prerequisites

- Rust and Cargo from the repository toolchain;
- Docker with the pinned `nixos/nix:2.20.6` image available;
- the exact LEZ v0.2.0 `sequencer_service` binary; and
- a matching sequencer JSON configuration.

The first Basecamp run may download pinned Nix inputs. Its named container is
kept running as a reusable local store, so subsequent runs do not repeat that
work.

The recursive claim stage is CPU intensive. On an unaccelerated laptop the
complete command can take hours; it uses a six-hour gate proof-age policy while
retaining the secure ten-minute CLI default for normal gates.

## One Command

From the repository root:

```bash
scripts/ci-submission-local.sh \
  /path/to/lez-v0.2.0/sequencer_service \
  /path/to/lez-v0.2.0/sequencer_config.json
```

An optional third argument selects the evidence directory. By default the
runner creates `artifacts/local-submission-<UTC timestamp>`.

The evidence directory contains:

- separate logs for Rust CI, the release build, standalone initialization, the
  full claim lifecycle, and Basecamp;
- a real balance receipt and recursively composed LEZ PPE proof;
- included deployment, initialization, and private-claim transaction hashes;
- fetched final GateState v3 and the holder-local access badge artifact after
  claim inclusion;
- exact gate/PPE user, paging, and total cycles, segments, proof bytes, and
  phase timings;
- the current LGX package;
- the official Basecamp integration output and non-empty desktop screenshot;
- tool versions, the source commit, worktree status, and sequencer binary hash;
- a top-level `SHA256SUMS` manifest.

Disposable presenter, gate-account signer, and private badge key files remain
in a mode-0700 temporary directory and are removed after successful evidence
export.

`scripts/basecamp-local.sh` can be run independently when only the Basecamp
package and integration suite need to be refreshed.
`scripts/ci-standalone-claim.sh` independently reproduces the full exact-node
claim lifecycle and sanitized public evidence.

## Evidence Boundary

These local results are auditable technical evidence and avoid dependence on
GitHub Actions execution. They do not turn a failed or unstarted hosted
workflow into a green default-branch check. LP-0005 explicitly requires green
CI on the default branch, so local evidence does not satisfy that item.

The standalone runner proves exact-version compatibility and the full
`RISC0_DEV_MODE=0` private claim lifecycle. Public deployment and
initialization are separately verified by:

```bash
scripts/ci-testnet-evidence.sh \
  artifacts/testnet-current \
  ee8068a772e5b928adfe4dbcc4752bf4ca6e514444b84222d8d8591d6c27c71b \
  f31b9c08215d2ec2d0cfd199db2713ca64ac781ad7640897f139b99db4d6f047
```

The public testnet evidence includes a real private claim, counter rotation,
badge binding, and stale replay denial. The network RPC still exposes no CU
field; that mandatory measurement gap remains explicit in [Implementation
Status](STATUS.md).
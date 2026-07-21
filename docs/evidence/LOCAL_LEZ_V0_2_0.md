# Local LEZ v0.2.0 Evidence

Date: 2026-07-21

This record covers the final ProofGate gate ELF against a standalone Logos
Execution Zone node built from tag `v0.2.0`, commit `a58fbce2`. The node ran
on `http://127.0.0.1:3040` with a 15-second block interval.

## Compatibility

- `checkHealth` returned success.
- `getAccount` returned authoritative public accounts.
- LEZ `v0.2.0` does not expose the newer `getProofsAndRoot` method.
- ProofGate's `getProofForCommitment` fallback returned a membership path and
  derived the same root through `MembershipProof::compute_root`.
- The ignored live integration test
  `pinned_v0_2_sequencer_membership_fallback` passed against this node.
- `sequencer-root` independently derived the selected node's root from its
  proof for the protocol default account anchor.
- `sendTransaction` accepted the root-bound deployment and initialization.

## Included Transactions

| Item | Value |
| --- | --- |
| Sequencer root | `d4e0961e5e2774178dad46069bfb7cd610b883417f7ce07475c9c61d3b45575f` |
| Root-bound program ID | `19936a0b1174095ae7c5978d1ee547741b81c01d6432e167c02a8a84fcec9a0d` |
| Deployment transaction | `d09c590a68e3754908d4bdb87e8dfc2152cf15636ae011d3d23a65094e293a66` |
| Gate account | `514a3e3cd0f1ea199afddcf48e1bf90372439cccc4090c2e95d815f03f94cd79` |
| Initialization transaction | `8f73d4a79715a1069ea2c3e9ee66ff94030cf2d9bd57ee27856fd4f66aa8bc96` |
| Inclusion observation | both transactions queryable by height `21` |
| Decoded GateState version | `2` |
| Decoded context hash | `c8136d53893ad946bec8cbe7a4bdb4734618d7290fa2ef685f18bacc06f55c95` |
| Decoded claim counter | `0` |

The authoritative state was fetched with:

```bash
proofgate on-chain fetch-state \
  --sequencer-url http://127.0.0.1:3040 \
  --gate-account-id-hex 514a3e3cd0f1ea199afddcf48e1bf90372439cccc4090c2e95d815f03f94cd79 \
  --output gate-state.json
```

The decoded state matched the submitted token owner, token definition,
threshold `100`, context hash, challenge nonce, claim counter, and the
independently queried commitment root.

## Reproducible Integration

`scripts/ci-standalone-lez.sh` starts a clean temporary node, queries the
root, runs the ignored live membership test, deploys the actual embedded ELF,
initializes a root-bound GateState, waits for both transactions, fetches the
authoritative state, and checks its version, root, and counter. That clean run
also completed on 2026-07-21. The script is wired into the
`Standalone LEZ v0.2.0` GitHub Actions job.

## Ownership Finding

An earlier development ELF set `account.program_owner` directly inside the
SPEL guest. The v0.2.0 executor rejected initialization with
`ModifiedProgramOwner`. LEZ expects an initialization guest to leave the
pre-claim owner unchanged and return `Claim::Authorized`; the protocol applies
the program owner after validating execution.

The final guest therefore writes state but does not mutate the owner. A real
guest-executor regression test now asserts both conditions:

- the post-state still has `DEFAULT_PROGRAM_ID` before claim application; and
- the account requests `Some(Claim::Authorized)`.

The same rule applies when creating an access badge. Existing gate ownership
remains unchanged, while the new badge remains default-owned in guest output
until LEZ applies authorization.

## Negative Check

`fetch-badge` was run against a fresh, uninitialized account. It rejected the
account because its authoritative owner was not the final balance-gate program.
This confirms the command does not decode arbitrary account bytes as an access
badge.

## Boundary

This evidence proves exact-version compatibility, independent root
acquisition, program deployment, root-bound initialization inclusion, ownership
behavior, and authoritative state decoding. It does not claim private claim
inclusion, an issued badge, compute units, testnet deployment, or transaction
cost. Those require a matching private token holder plus recursive composition
inside ProofGate's ten-minute validity window and remain separately tracked.

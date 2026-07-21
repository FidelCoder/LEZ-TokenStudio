# Local LEZ v0.2.0 Evidence

Date: 2026-07-20

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
- `sendTransaction` accepted the final deployment and initialization.

## Included Transactions

| Item | Value |
| --- | --- |
| Final program ID | `e776135f1f7ebf2dd810c232bd2d3cd75217b44407e12df1c0b1f5fbcf031337` |
| Deployment transaction | `61392cd68030c6d4b183dd9628885a683aa80cbaff6a2d72c5af1f80a344c276` |
| Deployment block | `79` |
| Gate account | `c4dc2f3eea999dbeccf2f4c04c881874b2cc4d5cae231c8caca10034830cf4de` |
| Initialization transaction | `0bd827648a9a5095cf8fd453c234c872e591313f0dbc629a1672145a50bc9d39` |
| Initialization block | `84` |
| Decoded context hash | `c8136d53893ad946bec8cbe7a4bdb4734618d7290fa2ef685f18bacc06f55c95` |
| Decoded claim counter | `0` |

The authoritative state was fetched with:

```bash
proofgate on-chain fetch-state \
  --sequencer-url http://127.0.0.1:3040 \
  --gate-account-id-hex c4dc2f3eea999dbeccf2f4c04c881874b2cc4d5cae231c8caca10034830cf4de \
  --output gate-state.json
```

The decoded state matched the submitted token owner, token definition,
threshold `100`, context hash, challenge nonce, and claim counter.

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

This evidence proves exact-version compatibility, program deployment,
initialization inclusion, ownership behavior, and authoritative state decoding.
It does not claim private claim inclusion, an issued badge, compute units,
testnet deployment, or transaction cost. Those require recursive composition
inside ProofGate's ten-minute validity window and remain separately tracked.

# Implementation Status

## Current Position

All repository-owned technical paths are implemented and validated without
depending on GitHub Actions. A real `RISC0_DEV_MODE=0` private claim is now
included on the official LEZ testnet. The repository is not yet ready for an
LP-0005 submission because the official criteria still require supported
network CU evidence, green default-branch CI, and a narrated video.

## Completed Technical Work

- Exact private account commitment and Merkle membership proof.
- Real `RISC0_DEV_MODE=0` threshold proof generation and verification.
- Context, authorized-root, fresh-challenge, presenter-key, and replay binding.
- SPEL on-chain verifier with GateState v3 and encrypted private access badge.
- Canonical LEZ private transaction composition and inclusion checks.
- Encrypted Logos Chat transport and sender-bound GroupV2 admission.
- Proposal-bound private governance integration with persistent challenges and
  one-pseudonym-one-vote enforcement.
- Basecamp GUI, downloadable LGX, official Qt tests, CLI/SDK, IDL, and local
  reproducible runners.
- Read-only public-testnet evidence collector with transaction inclusion,
  GateState version/counter assertions, and SHA-256 manifests.

## Verified Public Testnet

On 2026-07-28 the official `https://testnet.lez.logos.co` sequencer included:

- program ID
  `a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa`;
- deployment transaction
  `7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1`;
- private-token transaction
  `70740bfb7b701f41a7e5c99921114194ac865986e4860193f35e50a260bd0c0a`;
- proof-root-bound GateState v3 account
  `4f7b5ec8898c8509b4b18f39c646a838d37b6c00e7ffe251c410b91645640eb0`;
- initialization transaction
  `5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978`;
- private claim transaction
  `8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de`.

The fetched final state is GateState v3 with `claim_counter = 1`; the nonce
rotated, badge context/presenter/timestamp bindings matched, and stale replay
was denied before recomposition. The checksummed public bundle is
`artifacts/testnet-claim-current`. Wallet storage, account snapshots, presenter
keys, badge keys, and the holder-local badge are excluded.

The live recursive claim took 5,783,248 ms: 3,866,660 ms for the gate proof and
1,916,587 ms for the official PPE proof. It used 3,670,016 gate guest cycles and
1,048,576 PPE guest cycles, with a 223,970-byte balance receipt and a
230,611-byte PPE proof.

## Final Local Evidence

The exact LEZ v0.2 lifecycle independently includes a real 223,970-byte balance
receipt, 230,611-byte PPE proof, private claim inclusion, counter `0 -> 1`,
private badge claim `1`, nonce rotation, stale replay denial, and state
persistence after node restart. Recursive composition took 5,718,392 ms on the
recorded unaccelerated machine.

Current Logos Chat evidence reconstructed ten chunks plus one manifest,
admitted the bound sender to GroupV2, and denied forwarding/replay. The final
Basecamp package passed all four official Qt tests and rendered a non-empty
1024 by 768 frame.

## Mandatory Work Before Submission

These items are mandatory official outcomes, not optional polish:

- Document a supported testnet/devnet CU or gas cost for each on-chain
  operation. The current official RPC returns the serialized transaction and
  block ID but exposes no CU field, and current LEZ source has no cost-metadata
  RPC. Local Risc0 cycles must not be mislabeled as network CU.
- Make CI green on the public repository default branch. Local CI is
  reproducible evidence but does not replace this explicit hosted status. The
  current jobs do not start because the GitHub account is locked by a billing
  issue; no repository code executes before the failure.
- Record and link the mandatory narrated video showing terminal proof
  generation with `RISC0_DEV_MODE=0`, both verification paths, architecture,
  rationale, and key decisions.
- Publish the video link, owner-review the final write-up, and open the Lambda
  Prize solution PR.

The official criteria are:
<https://github.com/logos-co/lambda-prize/blob/master/prizes/LP-0005.md>.

## Reproduction

```bash
scripts/ci-local.sh
scripts/ci-testnet-evidence.sh \
  artifacts/testnet-claim-current \
  4f7b5ec8898c8509b4b18f39c646a838d37b6c00e7ffe251c410b91645640eb0 \
  5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978 \
  8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de
scripts/ci-standalone-claim.sh \
  /path/to/lez-v0.2.0/sequencer_service \
  /path/to/lez-v0.2.0/sequencer_config.json
scripts/basecamp-local.sh
scripts/video-preflight.sh
```

See [Public Testnet Evidence](evidence/LIVE_LEZ_TESTNET.md),
[Transaction Cost Evidence](evidence/LEZ_TESTNET_COSTS.md),
[Benchmarks](BENCHMARKS.md), [External Integrator
Guide](EXTERNAL_INTEGRATOR_GUIDE.md), and [Offline
Validation](OFFLINE_VALIDATION.md). The recording sequence is in [Video Demo
Guide](VIDEO_DEMO.md).

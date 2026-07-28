# Official Basecamp Integration Evidence

Current-package validation completed on 2026-07-28 with
`scripts/basecamp-local.sh`. The package includes the private badge key/ID
workflow and was built and tested without GitHub Actions.

## Pinned Inputs

| Input | Revision |
| --- | --- |
| `logos-module-builder` | `8a91e486b276cde0a806e683e8f2d6b785b2c91c` |
| `lez_core` | `033f807a3b1690fb2f8e661cb8067646b8a98242` |
| `chat_module` | `afb965589afb193a8559faf911233221a681af80` |
| `delivery_module` | `794c21cbe177bdea16d4907468eaf52d4282dda7` |

The reusable runner uses the pinned `nixos/nix:2.20.6` base image, one Nix
build job, and two cores. The final run reused local image
`proofgate-basecamp-nix:current` with image ID
`sha256:203fa62a3cb14566a3f98a68a4644db139bf5eddfd2c793300d2f57e0664416e`.

## Result

The official Logos Qt integration framework ran four tests:

1. The ProofGate backend loaded and connected.
2. Token and gate controls were present.
3. Gate, Prove, Verify, Messaging, and On-chain workflows were navigable.
4. The offscreen desktop render had positive dimensions and non-empty pixels.

Result: **4 passed, 0 failed**.

The integration host loaded `capability_module`, `lez_core`,
`delivery_module`, `chat_module`, and the ProofGate plugin. The rendered PNG
is 1024 by 768 pixels.

A separate repository-local source contract check verifies that the current
QML calls `private-account-generate` and `private-account-id`, supplies
`--badge-key` to composition and submission, writes `--output-badge` for
both paths, and never invokes the obsolete public `fetch-badge` workflow. It
runs from `scripts/ci-local.sh`; it is intentionally outside the official
runtime-test directory because the Nix runtime derivation does not include the
source tree.

## Package

The LGX manifest is version `0.1.0`, type `ui_qml`, and declares:

```json
["lez_core", "delivery_module", "chat_module"]
```

| Final local artifact | Bytes | SHA-256 |
| --- | ---: | --- |
| `logos-tokenstudio_proofgate_ui-module.lgx` | 765,585 | `dc6938044ae38805bf8cbefde908a7467d4392e70a12cfd2d4bb529911ee5145` |
| `integration/app-data/proofgate-desktop.png` | 47,633 | `a3017630cf163b77c0b1c2e6e56669808124ab854f50cafbcabddaab355161ad` |

The resolved outputs were:

- LGX: `/nix/store/0f7smb6xfygp16q8ykhsvdy36xw4ybws-logos-tokenstudio_proofgate_ui-module-lgx-0.1.0`
- integration: `/nix/store/gcjwqcmwjrb4scmhkzy7fp895aysxpyk-tokenstudio_proofgate_ui-integration-test`

Both artifact hashes verify against
`artifacts/local-submission-final/basecamp/SHA256SUMS`. The package is ready
for the narrated full-client demo; recording that demo is submission work, not
an unimplemented technical path.

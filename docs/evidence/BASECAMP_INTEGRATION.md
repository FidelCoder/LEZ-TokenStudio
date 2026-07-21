# Official Basecamp Integration Evidence

Historical validation completed on 2026-07-21 for repository revision
`18ecd373aeaa80bc74103b34b71d22ef73029cb2`. This record does not claim that
the later trusted-root UI revision has completed the same package run.

## Pinned Inputs

| Input | Revision |
| --- | --- |
| `logos-module-builder` | `8a91e486b276cde0a806e683e8f2d6b785b2c91c` |
| `lez_core` | `033f807a3b1690fb2f8e661cb8067646b8a98242` |
| `chat_module` | `afb965589afb193a8559faf911233221a681af80` |
| `delivery_module` | `794c21cbe177bdea16d4907468eaf52d4282dda7` |

## Commands

```bash
cd apps/basecamp-tokenstudio
nix build .#lgx -L -o result-lgx
nix build .#integration-test -L -o result-integration \
  --option max-jobs 1 --option cores 2
```

The integration run used the official Logos Qt test framework. It loaded
`capability_module`, `lez_core`, `delivery_module`, `chat_module`, and the
ProofGate backend before spawning the view host.

## Result

Four tests passed and zero failed:

1. The backend loads and connects.
2. Token and gate controls are present.
3. Gate, Prove, Verify, Messaging, and On-chain workflows are navigable.
4. The offscreen desktop render has positive dimensions and a non-empty image.

The rendered PNG is 1024 by 768 pixels. This test also validates the explicit
`delivery_module` dependency required by `chat_module`. `TMPDIR=/tmp` is set in
CI mode because the default Nix build path exceeds the Unix-domain socket path
limit used by the Qt view host.

## Current Revision Boundary

The current source adds trusted/authorized commitment-root fields and
`Read sequencer root` actions to the Prove, Messaging, and On-chain views.
The source test now asserts those controls. Rust/QML command wiring is covered
locally, but the current revision still requires:

1. `nix build .#lgx`;
2. `nix build .#integration-test`; and
3. installation plus desktop/narrow-window review in the full Basecamp client.

The lock file's `logos-delivery` entry retains the same revision but corrects
its source NAR hash. Rebuilding the full official dependency graph requires
network-enabled Nix execution; that execution has not been claimed here.

## Package

The resulting LGX manifest is version `0.1.0`, type `ui_qml`, and declares:

```json
["lez_core", "delivery_module", "chat_module"]
```

| Local release artifact | SHA-256 |
| --- | --- |
| `TokenStudio-ProofGate-v0.1.0.lgx` | `0b25838b07c54be9fae8bb614bbe2a927c80e6a378dc4350886a6ecd2c66d3b9` |
| `TokenStudio-ProofGate-desktop.png` | `9c81817ca7a4422c310bb24e55b921bdfeb52a09faaa99023454af9fb806f282` |

The manifest root hash is
`0b0fe6215972cc7f301cc1f2169e0593bc09d1d2281ad18c0f2522b44c8416ce`.
These hashes identify only the historical artifacts for the revision stated at
the top of this file. They are kept outside the Git repository for audit
comparison and must not be published as the current trusted-root build.

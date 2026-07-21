# Official Basecamp Integration Evidence

Validated on 2026-07-21 with the locked Nix graph in
`apps/basecamp-tokenstudio/flake.lock`.

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
The release artifacts are kept outside the Git repository pending owner review
and publication. Full-client installation, interactive visual review, and a
narrow-window screenshot remain release evidence rather than claims made by
this offscreen integration run.

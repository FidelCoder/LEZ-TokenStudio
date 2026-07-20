# TokenStudio ProofGate Basecamp Module

This `ui_qml` module exposes the complete ProofGate operator workflow inside
Basecamp:

- create, select, and mint LEZ tokens;
- configure token-specific threshold gates;
- generate presenter keys and real Risc0 balance proofs;
- issue challenges and create signed presentations;
- verify presentations locally with persistent replay protection;
- send and receive chunked presentations through `chat_module`;
- admit a sender-bound holder to a GroupV2 conversation; and
- deploy, initialize, simulate, compose, sign, and submit the SPEL on-chain
  access-badge flow.

## Architecture

The module follows the current universal Logos `ui_qml` pattern:

- `metadata.json` declares `logos_execution_zone` and `chat_module` runtime
  dependencies;
- `src/proofgate_ui.rep` is the Qt Remote Objects contract;
- `ProofGateUiBackend` asynchronously launches the audited `proofgate` binary;
- `src/qml/ProofGateView.qml` is the Basecamp view.

The backend uses `QProcess` argument lists and never invokes a shell. It forces
`RISC0_DEV_MODE=0`, drains stdout/stderr while long proofs run, rejects
concurrent operations, and supports cancellation. Presenter secrets remain in
permission-restricted files managed by the Rust CLI; they are never returned
as QML values. LEZ gate and badge signer files use the same restricted-file
boundary.

## Build

Build the Rust CLI first:

```bash
cargo build --release -p proofgate
```

Then build the module with the official Logos module builder:

```bash
cd apps/basecamp-tokenstudio
nix build .#lgx
```

Install the resulting `.lgx` with `lgpm`. In the module header, set the
ProofGate binary to the absolute path of `target/release/proofgate` and select
**Use binary**.

For a non-Nix developer build, set `LOGOS_MODULE_BUILDER_ROOT` and run CMake in
the usual out-of-tree build directory. Qt 6 and the module builder's generated
SDK are required.

## Runtime Notes

Proof generation and recursive LEZ composition can take minutes on commodity
hardware. The backend runs them asynchronously, streams accumulated output to
the result pane, and keeps the Basecamp UI responsive. Closing Basecamp or
selecting **Cancel** terminates the child process, so an interrupted proof must
be restarted.

The Messaging controls expect initialized, online Logos Core instances and
existing direct/group conversation IDs. See `docs/LOGOS_MESSAGING.md` and
`demos/token-gated-chat/README.md` for the complete two-instance flow.

## Local Validation Boundary

The Rust command contracts behind every control are covered by workspace tests
and strict Clippy. Packaging this module requires Nix, CMake, Qt, and the Logos
module builder; those tools are not part of the Rust workspace itself.

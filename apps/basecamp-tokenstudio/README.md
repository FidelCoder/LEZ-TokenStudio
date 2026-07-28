# TokenStudio ProofGate Basecamp Module

This `ui_qml` module exposes the complete ProofGate operator workflow inside
Basecamp:

- create, select, and mint LEZ tokens;
- configure token-specific threshold gates;
- generate presenter keys and real Risc0 balance proofs;
- read a verifier-owned sequencer root, issue root-bound challenges, and create
  signed presentations;
- verify presentations locally with persistent replay protection;
- send and receive chunked presentations through `chat_module`;
- admit a sender-bound holder to a GroupV2 conversation; and
- read an operator-owned sequencer root, then deploy and initialize the public
  gate, generate a private badge identity, compose its encrypted output, and
  submit the SPEL private transaction.

## Architecture

The module follows the current universal Logos `ui_qml` pattern:

- `metadata.json` declares `lez_core`, `delivery_module`, and `chat_module`
  runtime dependencies;
- `src/proofgate_ui.rep` is the Qt Remote Objects contract;
- `TokenstudioProofgateUiBackend` asynchronously launches the audited
  `proofgate` binary;
- `src/qml/ProofGateView.qml` is the Basecamp view.

The backend uses `QProcess` argument lists and never invokes a shell. It forces
`RISC0_DEV_MODE=0`, drains stdout/stderr while long proofs run, rejects
concurrent operations, and supports cancellation. Presenter secrets remain in
permission-restricted files managed by the Rust CLI; they are never returned
as QML values. LEZ gate signer, private badge, and presenter key files use the
same restricted-file boundary.

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

The Rust command contracts and QML source assertions behind every control are
covered by workspace tests and strict Clippy. The local package runner builds
the locked official module builder, Qt, `lez_core`, `delivery_module`, and
`chat_module` graph, then runs the official Logos Qt integration suite and
exports a non-empty desktop screenshot.

Run `scripts/basecamp-local.sh` from the repository root to reproduce the LGX
and integration artifacts. See `docs/evidence/BASECAMP_INTEGRATION.md` for
the current hashes and remaining narrated-demo boundary.

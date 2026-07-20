# Token-Gated Chat Demo

This demo drives two real `chat_module` instances through `logoscore`:

- the verifier issues and sends an admission-bound challenge over encrypted Chat;
- the holder receives it, then signs and sends a chunked Risc0 presentation over Logos Messaging;
- the verifier reconstructs and verifies it locally;
- the verifier calls `add_group_member` for the bound holder address;
- the script waits until `list_group_members` confirms admission;
- wrong-verifier forwarding and replay are denied.

## Prerequisites

Follow the official `logos-chat-module` headless exchange setup to build
`logoscore`, install `chat_module` plus `delivery_module`, start two isolated
daemons, initialize both modules, and open a direct conversation. The verifier
must also own a GroupV2 conversation that the holder has not joined yet.

Build ProofGate in release mode and create a fresh real balance proof:

```bash
cargo build --release -p proofgate
RISC0_DEV_MODE=0 target/release/proofgate prove <arguments>
```

The proof must still satisfy ProofGate's 10-minute freshness policy when the
receiver verifies it.

## Required Environment

```bash
export GATE=/absolute/path/gate.json
export PROOF=/absolute/path/proof.json
export PRESENTER_KEY=/absolute/path/presenter.json
export HOLDER_CONFIG_DIR=/absolute/path/holder-config
export VERIFIER_CONFIG_DIR=/absolute/path/verifier-config
export HOLDER_CONVERSATION_ID=<holder-side-direct-conversation-id>
export VERIFIER_CONVERSATION_ID=<verifier-side-direct-conversation-id>
export TARGET_GROUP_ID=<verifier-owned-group-id>
```

Optional overrides:

```bash
export PROOFGATE_BIN=target/release/proofgate
export LOGOSCORE_BIN=logoscore
export MEMBER_ADDRESS=<holder-get_address-result>
export WORK_DIR=/tmp/proofgate-chat-demo
```

Run:

```bash
demos/token-gated-chat/run.sh
```

The script does not create daemons or silently select conversations. Those are
long-lived user identities and groups, so their paths and IDs are explicit.

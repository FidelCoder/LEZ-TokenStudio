# Logos Messaging Integration

ProofGate transports private balance presentations through the official
`chat_module` API. Messages are end-to-end encrypted by Logos Chat and carried
by its `delivery_module` dependency. Verification remains local: the receiver
does not call a hosted ProofGate service or expose the receipt over HTTP.

The complete flow has been validated against two isolated Logos Core daemons
on `logos.test`: a 223,970-byte real receipt crossed Chat in ten chunks plus one
manifest, verified locally, and resulted in observed GroupV2 membership. See
the [live evidence record](evidence/LIVE_LOGOS_MESSAGING.md).

## Runtime Boundary

The Rust adapter invokes the headless Logos Core CLI without a shell:

```text
logoscore --config-dir <instance> call chat_module send_message <conversation> <content>
logoscore --config-dir <instance> call chat_module get_messages <conversation>
logoscore --config-dir <instance> call chat_module add_group_member <group> <address>
```

This is the same request/response boundary used by the official
`logos-chat-module` doctests. A Basecamp consumer can feed `message_received`
event content into the same reassembler instead of polling.

## Transfer Protocol

A real Risc0 receipt is larger than a prudent single chat message. Protocol
`proofgate/attestation-transfer` version 1 therefore sends:

1. Base64 chunks of at most 48 KiB decoded data each; then
2. one manifest as the transfer commit.

The default chunk size is 32 KiB. A transfer is bounded to 4 MiB and 128
chunks. The manifest commits to:

- a random 128-bit transfer ID;
- exact serialized envelope byte length;
- chunk count;
- SHA-256 of the complete envelope;
- gate context hash; and
- verifier challenge digest.

The receiver ignores ordinary chat content and other protocols. It rejects
conflicting manifests/chunks, missing chunks, invalid indexes, oversized data,
hash mismatch, context mismatch, challenge mismatch, and envelopes that do not
declare `LogosMessaging` transport. No partially received payload is passed to
the cryptographic verifier.

## Chat Admission Binding

For a token-gated group, ProofGate preserves the gate's fixed verifier identity
and binds admission into the 32-byte challenge nonce:

```text
random[0..16] || sha256(domain || challenge || random || group || member)[0..16]
```

The binding includes challenge version, gate context, fixed verifier ID,
timestamps, a random 128-bit prefix, and length-prefixed group/member strings.
The holder's presenter signature therefore authorizes one exact member address
for one exact group without weakening the verifier identity check.
`messaging receive-verify` additionally requires the encrypted Chat message
sender to equal the requested member address. A proof forwarded by a different
chat identity is ignored, and changing the target group/member fails with
challenge error `1006`.

After receipt, journal, gate context, freshness, challenge, and presenter
signature verification all pass, ProofGate invokes `add_group_member`. The
chat module commits group membership asynchronously; callers should observe
`members_changed` or poll `list_group_members` for completion.

## Commands

Create an admission-bound challenge:

```bash
proofgate messaging admission-challenge \
  --gate gate.json \
  --commitment-root-hex "$COMMITMENT_ROOT" \
  --group-id "$GROUP_ID" \
  --member-address "$MEMBER_ADDRESS" \
  --ttl-ms 300000 \
  --output challenge.json
```

`COMMITMENT_ROOT` must be read by the verifier from its own trusted sequencer
endpoint, for example with `proofgate sequencer-root --sequencer-url
http://127.0.0.1:3040`. It must not be copied from the holder's proof.

Send it from the verifier and receive it on the holder identity:

```bash
proofgate messaging send-challenge \
  --logoscore-binary logoscore \
  --config-dir verifier \
  --conversation-id "$VERIFIER_CONVERSATION_ID" \
  --challenge challenge.json

proofgate messaging receive-challenge \
  --logoscore-binary logoscore \
  --config-dir holder \
  --conversation-id "$HOLDER_CONVERSATION_ID" \
  --expected-sender "$VERIFIER_ADDRESS" \
  --gate gate.json \
  --group-id "$GROUP_ID" \
  --member-address "$MEMBER_ADDRESS" \
  --output holder-challenge.json
```

Create and send a presentation from the holder's Logos Core instance:

```bash
proofgate present \
  --proof proof.json \
  --challenge holder-challenge.json \
  --presenter-key presenter.json \
  --transport logos-messaging \
  --output envelope.json

proofgate messaging send \
  --logoscore-binary logoscore \
  --config-dir holder \
  --conversation-id "$HOLDER_CONVERSATION_ID" \
  --envelope envelope.json
```

Receive, verify, and request admission from the gate owner's instance:

```bash
proofgate messaging receive-verify \
  --logoscore-binary logoscore \
  --config-dir verifier \
  --conversation-id "$VERIFIER_CONVERSATION_ID" \
  --transfer-id "$TRANSFER_ID" \
  --expected-sender "$MEMBER_ADDRESS" \
  --gate gate.json \
  --challenge challenge.json \
  --replay-cache replay-cache.json \
  --output verified-envelope.json \
  --admit-group-id "$GROUP_ID" \
  --admit-address "$MEMBER_ADDRESS"
```

`receive-verify` requires the exact challenge file retained by the verifier;
it rejects an envelope carrying any other challenge even when the holder has
validly signed that substitute. It writes the requested envelope file only
after local cryptographic verification succeeds. The replay cache is persisted
atomically.

## Validation

`cargo test -p proofgate-messaging` covers a receipt-sized 220 KB envelope,
bounded message sizes, round-trip reconstruction, incomplete transfers,
modified chunks, unrelated chat content, and unambiguous group/member binding.

Run `demos/token-gated-chat/run.sh` against two initialized, online
`chat_module` instances for the real encrypted network flow. The script verifies
group membership after admission and exercises forwarding and replay denials.

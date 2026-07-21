#!/usr/bin/env bash
set -euo pipefail

: "${GATE:?set GATE to a gate config JSON file}"
: "${PROOF:?set PROOF to a fresh real proof JSON file}"
: "${PRESENTER_KEY:?set PRESENTER_KEY to the matching presenter key file}"
: "${COMMITMENT_ROOT_HEX:?set COMMITMENT_ROOT_HEX from an independently trusted sequencer root}"
: "${HOLDER_CONFIG_DIR:?set HOLDER_CONFIG_DIR to the holder logoscore config directory}"
: "${VERIFIER_CONFIG_DIR:?set VERIFIER_CONFIG_DIR to the verifier logoscore config directory}"
: "${HOLDER_CONVERSATION_ID:?set HOLDER_CONVERSATION_ID to the holder direct conversation ID}"
: "${VERIFIER_CONVERSATION_ID:?set VERIFIER_CONVERSATION_ID to the verifier direct conversation ID}"
: "${TARGET_GROUP_ID:?set TARGET_GROUP_ID to the verifier-owned GroupV2 ID}"

PROOFGATE_BIN="${PROOFGATE_BIN:-target/release/proofgate}"
LOGOSCORE_BIN="${LOGOSCORE_BIN:-logoscore}"
WORK_DIR="${WORK_DIR:-/tmp/proofgate-chat-demo}"

command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }
test -x "$PROOFGATE_BIN" || { echo "ProofGate binary is not executable: $PROOFGATE_BIN" >&2; exit 1; }
command -v "$LOGOSCORE_BIN" >/dev/null || test -x "$LOGOSCORE_BIN" || {
  echo "logoscore binary is not executable: $LOGOSCORE_BIN" >&2
  exit 1
}

mkdir -p "$WORK_DIR"
CHALLENGE="$WORK_DIR/challenge.json"
HOLDER_CHALLENGE="$WORK_DIR/holder-challenge.json"
ENVELOPE="$WORK_DIR/envelope.json"
RECEIVED="$WORK_DIR/verified-envelope.json"
REPLAY_CACHE="$WORK_DIR/replay-cache.json"
FORWARD_CACHE="$WORK_DIR/forward-cache.json"

if [[ -z "${MEMBER_ADDRESS:-}" ]]; then
  MEMBER_ADDRESS=$("$LOGOSCORE_BIN" --config-dir "$HOLDER_CONFIG_DIR" \
    call chat_module get_address | jq -er '.result')
fi
if [[ -z "${VERIFIER_ADDRESS:-}" ]]; then
  VERIFIER_ADDRESS=$("$LOGOSCORE_BIN" --config-dir "$VERIFIER_CONFIG_DIR" \
    call chat_module get_address | jq -er '.result')
fi

echo "[1/8] Issue admission-bound verifier challenge"
"$PROOFGATE_BIN" messaging admission-challenge \
  --gate "$GATE" \
  --group-id "$TARGET_GROUP_ID" \
  --commitment-root-hex "$COMMITMENT_ROOT_HEX" \
  --member-address "$MEMBER_ADDRESS" \
  --ttl-ms 300000 \
  --output "$CHALLENGE"

echo "[2/8] Send challenge to the holder over encrypted Logos Chat"
"$PROOFGATE_BIN" messaging send-challenge \
  --logoscore-binary "$LOGOSCORE_BIN" \
  --config-dir "$VERIFIER_CONFIG_DIR" \
  --conversation-id "$VERIFIER_CONVERSATION_ID" \
  --challenge "$CHALLENGE"
"$PROOFGATE_BIN" messaging receive-challenge \
  --logoscore-binary "$LOGOSCORE_BIN" \
  --config-dir "$HOLDER_CONFIG_DIR" \
  --conversation-id "$HOLDER_CONVERSATION_ID" \
  --expected-sender "$VERIFIER_ADDRESS" \
  --gate "$GATE" \
  --group-id "$TARGET_GROUP_ID" \
  --member-address "$MEMBER_ADDRESS" \
  --timeout-ms 180000 \
  --output "$HOLDER_CHALLENGE"

echo "[3/8] Holder signs the real Risc0 proof presentation"
"$PROOFGATE_BIN" present \
  --proof "$PROOF" \
  --challenge "$HOLDER_CHALLENGE" \
  --presenter-key "$PRESENTER_KEY" \
  --transport logos-messaging \
  --output "$ENVELOPE"

echo "[4/8] Demonstrate forwarding denial for another verifier"
set +e
FORWARD_OUTPUT=$("$PROOFGATE_BIN" verify \
  --gate "$GATE" \
  --envelope "$ENVELOPE" \
  --challenge "$CHALLENGE" \
  --replay-cache "$FORWARD_CACHE" \
  --verifier-id "logos-chat:forwarded-copy" 2>&1)
FORWARD_STATUS=$?
set -e
if [[ $FORWARD_STATUS -eq 0 ]] || ! grep -Fq '[1008]' <<<"$FORWARD_OUTPUT"; then
  echo "expected forwarding denial [1008], got: $FORWARD_OUTPUT" >&2
  exit 1
fi
echo "Forwarded presentation denied [1008]"

echo "[5/8] Send chunked presentation over encrypted Logos Chat"
SEND_OUTPUT=$("$PROOFGATE_BIN" messaging send \
  --logoscore-binary "$LOGOSCORE_BIN" \
  --config-dir "$HOLDER_CONFIG_DIR" \
  --conversation-id "$HOLDER_CONVERSATION_ID" \
  --envelope "$ENVELOPE")
printf '%s\n' "$SEND_OUTPUT"
TRANSFER_ID=$(sed -n 's/^Transfer ID: //p' <<<"$SEND_OUTPUT")
test -n "$TRANSFER_ID" || { echo "send did not return a transfer ID" >&2; exit 1; }

echo "[6/8] Receive, verify locally, and request bound group admission"
"$PROOFGATE_BIN" messaging receive-verify \
  --logoscore-binary "$LOGOSCORE_BIN" \
  --config-dir "$VERIFIER_CONFIG_DIR" \
  --conversation-id "$VERIFIER_CONVERSATION_ID" \
  --transfer-id "$TRANSFER_ID" \
  --expected-sender "$MEMBER_ADDRESS" \
  --timeout-ms 180000 \
  --gate "$GATE" \
  --challenge "$CHALLENGE" \
  --replay-cache "$REPLAY_CACHE" \
  --output "$RECEIVED" \
  --admit-group-id "$TARGET_GROUP_ID" \
  --admit-address "$MEMBER_ADDRESS"

echo "[7/8] Wait for asynchronous GroupV2 membership commit"
ADMITTED=""
for _ in $(seq 1 40); do
  MEMBERS=$("$LOGOSCORE_BIN" --config-dir "$VERIFIER_CONFIG_DIR" \
    call chat_module list_group_members "$TARGET_GROUP_ID" 2>/dev/null || true)
  if jq -e --arg address "$MEMBER_ADDRESS" \
    '.result[]? | select(.address == $address)' <<<"$MEMBERS" >/dev/null 2>&1; then
    ADMITTED=1
    break
  fi
  sleep 3
done
test -n "$ADMITTED" || { echo "member admission was not observed" >&2; exit 1; }
echo "Member admitted: $MEMBER_ADDRESS"

echo "[8/8] Demonstrate persistent replay denial"
set +e
REPLAY_OUTPUT=$("$PROOFGATE_BIN" verify \
  --gate "$GATE" \
  --envelope "$RECEIVED" \
  --challenge "$CHALLENGE" \
  --replay-cache "$REPLAY_CACHE" 2>&1)
REPLAY_STATUS=$?
set -e
if [[ $REPLAY_STATUS -eq 0 ]] || ! grep -Fq '[1010]' <<<"$REPLAY_OUTPUT"; then
  echo "expected replay denial [1010], got: $REPLAY_OUTPUT" >&2
  exit 1
fi

echo "PASS: encrypted challenge and proof transport, local verification, bound admission, forwarding denial, replay denial"
echo "Artifacts: $WORK_DIR"

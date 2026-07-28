#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 2 ]]; then
  printf 'usage: %s <sequencer-binary> <sequencer-config>\n' "$0" >&2
  exit 2
fi

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SEQUENCER_BIN=$(realpath "$1")
SEQUENCER_CONFIG=$(realpath "$2")
PROOFGATE_BIN="$ROOT/target/release/proofgate"
SEQUENCER_URL="http://127.0.0.1:3040"
RUN_DIR=$(mktemp -d)
SEQUENCER_PID=""

cleanup() {
  if [[ -n "$SEQUENCER_PID" ]] && kill -0 "$SEQUENCER_PID" 2>/dev/null; then
    kill "$SEQUENCER_PID"
    wait "$SEQUENCER_PID" || true
  fi
}
trap cleanup EXIT

if curl --fail --silent \
  --header "content-type: application/json" \
  --data '{"jsonrpc":"2.0","id":1,"method":"checkHealth","params":[]}' \
  "$SEQUENCER_URL" >/dev/null 2>&1; then
  printf 'refusing to start: a healthy service already owns %s\n' \
    "$SEQUENCER_URL" >&2
  exit 1
fi

(
  cd "$RUN_DIR"
  exec "$SEQUENCER_BIN" "$SEQUENCER_CONFIG" \
    >"$RUN_DIR/sequencer.log" 2>&1
) &
SEQUENCER_PID=$!

healthy=0
for _ in $(seq 1 120); do
  if curl --fail --silent \
    --header "content-type: application/json" \
    --data '{"jsonrpc":"2.0","id":1,"method":"checkHealth","params":[]}' \
    "$SEQUENCER_URL" >/dev/null; then
    healthy=1
    break
  fi
  sleep 0.5
done
if [[ "$healthy" -ne 1 ]] || ! kill -0 "$SEQUENCER_PID" 2>/dev/null; then
  printf '%s\n' "standalone sequencer did not become healthy" >&2
  cat "$RUN_DIR/sequencer.log" >&2
  exit 1
fi

cd "$ROOT"
COMMITMENT_ROOT=$("$PROOFGATE_BIN" sequencer-root \
  --sequencer-url "$SEQUENCER_URL")
if [[ ! "$COMMITMENT_ROOT" =~ ^[0-9a-f]{64}$ ]]; then
  printf 'invalid sequencer root: %s\n' "$COMMITMENT_ROOT" >&2
  exit 1
fi

LEZ_SEQUENCER_URL="$SEQUENCER_URL" cargo test -p attestation-prover \
  pinned_v0_2_sequencer_membership_fallback -- --ignored --nocapture

DEPLOYMENT=$("$PROOFGATE_BIN" on-chain deploy \
  --sequencer-url "$SEQUENCER_URL")
PROGRAM_ID=$(printf '%s\n' "$DEPLOYMENT" | awk '/Program ID:/ {print $3}')
DEPLOYMENT_TX=$(printf '%s\n' "$DEPLOYMENT" | awk '/Transaction hash:/ {print $3}')
EXPECTED_PROGRAM_ID=$("$PROOFGATE_BIN" on-chain program-id)
test "$PROGRAM_ID" = "$EXPECTED_PROGRAM_ID"

"$PROOFGATE_BIN" on-chain account-generate \
  --output "$RUN_DIR/gate-account.json" >"$RUN_DIR/gate-account.out"
GATE_ACCOUNT_ID=$(awk '/Account ID hex:/ {print $4}' "$RUN_DIR/gate-account.out")
CHALLENGE_NONCE=$(openssl rand -hex 32)
"$PROOFGATE_BIN" on-chain init \
  --gate examples/gates/founders.json \
  --commitment-root-hex "$COMMITMENT_ROOT" \
  --challenge-nonce-hex "$CHALLENGE_NONCE" \
  --output "$RUN_DIR/gate-state.json"
INITIALIZATION=$("$PROOFGATE_BIN" on-chain initialize-submit \
  --sequencer-url "$SEQUENCER_URL" \
  --state "$RUN_DIR/gate-state.json" \
  --gate-key "$RUN_DIR/gate-account.json")
INITIALIZATION_TX=$(printf '%s\n' "$INITIALIZATION" | awk '/Transaction hash:/ {print $3}')

wait_for_transaction() {
  local hash=$1
  local payload
  local response
  payload=$(jq -cn --arg hash "$hash" \
    '{jsonrpc:"2.0",id:1,method:"getTransaction",params:[$hash]}')
  for _ in $(seq 1 90); do
    response=$(curl --fail --silent --show-error \
      --header "content-type: application/json" \
      --data "$payload" "$SEQUENCER_URL")
    if jq -e '.result != null' <<<"$response" >/dev/null; then
      return 0
    fi
    sleep 1
  done
  printf 'transaction was not included: %s\n' "$hash" >&2
  return 1
}

wait_for_transaction "$DEPLOYMENT_TX"
wait_for_transaction "$INITIALIZATION_TX"

"$PROOFGATE_BIN" on-chain fetch-state \
  --sequencer-url "$SEQUENCER_URL" \
  --gate-account-id-hex "$GATE_ACCOUNT_ID" \
  --output "$RUN_DIR/fetched-gate-state.json"

test "$(jq -r '.version' "$RUN_DIR/fetched-gate-state.json")" = "3"
test "$(jq -r '.claim_counter' "$RUN_DIR/fetched-gate-state.json")" = "0"
test "$(jq -c '.commitment_root' "$RUN_DIR/fetched-gate-state.json")" = \
  "$(jq -c '.commitment_root' "$RUN_DIR/gate-state.json")"

printf 'Standalone LEZ integration passed\n'
printf 'Program ID: %s\n' "$PROGRAM_ID"
printf 'Commitment root: %s\n' "$COMMITMENT_ROOT"
printf 'Deployment transaction: %s\n' "$DEPLOYMENT_TX"
printf 'Initialization transaction: %s\n' "$INITIALIZATION_TX"

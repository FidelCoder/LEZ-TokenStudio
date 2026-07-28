#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 3 || $# -gt 4 ]]; then
  printf 'usage: %s <evidence-dir> <gate-account-id-hex> <initialization-tx> [claim-tx]\n' "$0" >&2
  exit 2
fi

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
EVIDENCE_DIR=$1
GATE_ACCOUNT_ID=$2
INITIALIZATION_TX=$3
CLAIM_TX=${4:-}
SEQUENCER_URL=${LEZ_TESTNET_URL:-https://testnet.lez.logos.co}
PROOFGATE_BIN=${PROOFGATE_BIN:-"$ROOT/target/release/proofgate"}
DEPLOYMENT_TX=7a3b9b1e61bcb1d1559937e07fadcabee9965b50a44d9e1f82b55d1a3fd535a1
EXPECTED_PROGRAM_ID=a2daba934bd6993d8c673c363ca75a24733780f26341b1b95e3d1bd81bf634aa

[[ "$SEQUENCER_URL" == https://* ]] || {
  printf 'refusing non-HTTPS testnet endpoint: %s\n' "$SEQUENCER_URL" >&2
  exit 1
}
[[ "$GATE_ACCOUNT_ID" =~ ^[0-9a-f]{64}$ ]] || {
  printf 'invalid gate account ID\n' >&2
  exit 1
}
[[ "$INITIALIZATION_TX" =~ ^[0-9a-f]{64}$ ]] || {
  printf 'invalid initialization transaction hash\n' >&2
  exit 1
}
if [[ -n "$CLAIM_TX" && ! "$CLAIM_TX" =~ ^[0-9a-f]{64}$ ]]; then
  printf 'invalid claim transaction hash\n' >&2
  exit 1
fi
command -v curl >/dev/null
command -v jq >/dev/null
command -v sha256sum >/dev/null
[[ -x "$PROOFGATE_BIN" ]] || {
  printf 'missing release CLI: %s\n' "$PROOFGATE_BIN" >&2
  exit 1
}

mkdir -p "$EVIDENCE_DIR/rpc"
EVIDENCE_DIR=$(realpath "$EVIDENCE_DIR")

rpc() {
  local method=$1
  local params=$2
  local output=$3
  local payload
  payload=$(jq -cn --arg method "$method" --argjson params "$params" \
    '{jsonrpc:"2.0",id:1,method:$method,params:$params}')
  curl --fail-with-body --silent --show-error --max-time 30 \
    --header 'content-type: application/json' \
    --data "$payload" "$SEQUENCER_URL" >"$output"
  jq -e 'has("result") and (.error == null)' "$output" >/dev/null
}

wait_for_transaction() {
  local label=$1
  local hash=$2
  local raw="$EVIDENCE_DIR/rpc/$label.raw.json"
  local summary="$EVIDENCE_DIR/rpc/$label.json"
  local included=0

  for _ in $(seq 1 30); do
    rpc getTransaction "$(jq -cn --arg hash "$hash" '[$hash]')" "$raw"
    if jq -e '.result != null' "$raw" >/dev/null; then
      included=1
      break
    fi
    sleep 2
  done
  [[ "$included" -eq 1 ]] || {
    printf 'transaction not included: %s\n' "$hash" >&2
    exit 1
  }

  jq -n \
    --arg hash "$hash" \
    --arg response_sha256 "$(sha256sum "$raw" | cut -d ' ' -f 1)" \
    --argjson serialized_result_chars "$(jq -r '.result | length' "$raw")" \
    '{transaction_hash:$hash,included:true,serialized_result_chars:$serialized_result_chars,response_sha256:$response_sha256}' \
    >"$summary"
  perl -e 'unlink $ARGV[0] or die "$ARGV[0]: $!"' "$raw"
}

rpc checkHealth '[]' "$EVIDENCE_DIR/rpc/health.json"
jq -e '.result == null' "$EVIDENCE_DIR/rpc/health.json" >/dev/null

PROGRAM_ID=$("$PROOFGATE_BIN" on-chain program-id)
[[ "$PROGRAM_ID" == "$EXPECTED_PROGRAM_ID" ]] || {
  printf 'unexpected embedded program ID: %s\n' "$PROGRAM_ID" >&2
  exit 1
}

wait_for_transaction deployment "$DEPLOYMENT_TX"
wait_for_transaction initialization "$INITIALIZATION_TX"
if [[ -n "$CLAIM_TX" ]]; then
  wait_for_transaction claim "$CLAIM_TX"
fi

"$PROOFGATE_BIN" on-chain fetch-state \
  --sequencer-url "$SEQUENCER_URL" \
  --gate-account-id-hex "$GATE_ACCOUNT_ID" \
  --output "$EVIDENCE_DIR/gate-state.json" \
  >"$EVIDENCE_DIR/fetch-state.log"
EXPECTED_COUNTER=0
if [[ -n "$CLAIM_TX" ]]; then
  EXPECTED_COUNTER=1
fi
jq -e --argjson expected_counter "$EXPECTED_COUNTER" \
  '.version == 3 and .claim_counter == $expected_counter' \
  "$EVIDENCE_DIR/gate-state.json" >/dev/null
"$PROOFGATE_BIN" sequencer-root \
  --sequencer-url "$SEQUENCER_URL" \
  >"$EVIDENCE_DIR/current-commitment-root.txt"

{
  printf 'captured_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'endpoint=%s\n' "$SEQUENCER_URL"
  printf 'program_id=%s\n' "$PROGRAM_ID"
  printf 'deployment_transaction=%s\n' "$DEPLOYMENT_TX"
  printf 'gate_account_id=%s\n' "$GATE_ACCOUNT_ID"
  printf 'initialization_transaction=%s\n' "$INITIALIZATION_TX"
  printf 'claim_transaction=%s\n' "$CLAIM_TX"
  printf 'gate_state_version=%s\n' \
    "$(jq -r '.version' "$EVIDENCE_DIR/gate-state.json")"
  printf 'claim_counter=%s\n' \
    "$(jq -r '.claim_counter' "$EVIDENCE_DIR/gate-state.json")"
  printf 'max_proof_age_ms=%s\n' \
    "$(jq -r '.max_proof_age_ms' "$EVIDENCE_DIR/gate-state.json")"
  printf 'rpc_compute_unit_field=not_exposed\n'
} >"$EVIDENCE_DIR/summary.txt"

(
  cd "$EVIDENCE_DIR"
  find . -type f ! -name SHA256SUMS -print0 |
    sort -z |
    xargs -0 sha256sum >SHA256SUMS
  sha256sum --check SHA256SUMS
)

printf 'Official LEZ testnet evidence verified: %s\n' "$EVIDENCE_DIR"

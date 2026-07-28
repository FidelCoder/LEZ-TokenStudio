#!/usr/bin/env bash
set -euo pipefail
umask 077

if [[ $# -lt 2 || $# -gt 3 ]]; then
  printf 'usage: %s <sequencer-binary> <sequencer-config> [evidence-dir]\n' "$0" >&2
  exit 2
fi

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SEQUENCER_BIN=$(realpath "$1")
SEQUENCER_CONFIG=$(realpath "$2")
OUTPUT_DIR=${3:-"$ROOT/artifacts/standalone-claim-current"}
SEQUENCER_URL="http://127.0.0.1:3040"
RUN_DIR=$(mktemp -d /tmp/proofgate-standalone-claim-XXXXXX)
SEQUENCER_PID=""

cleanup() {
  local status=$?

  trap - EXIT
  if [[ -n "$SEQUENCER_PID" ]] && kill -0 "$SEQUENCER_PID" 2>/dev/null; then
    kill "$SEQUENCER_PID"
    wait "$SEQUENCER_PID" || true
  fi
  if [[ "$status" -eq 0 ]]; then
    case "$RUN_DIR" in
      /tmp/proofgate-standalone-claim-*) rm -rf -- "$RUN_DIR" ;;
      *) printf 'refusing to remove unexpected temporary path: %s\n' "$RUN_DIR" >&2 ;;
    esac
  else
    printf 'failed lifecycle work remains at %s\n' "$RUN_DIR" >&2
  fi
  exit "$status"
}
trap cleanup EXIT

for command in base64 cargo cmp curl jq openssl sha256sum stat wc; do
  command -v "$command" >/dev/null || {
    printf '%s is required\n' "$command" >&2
    exit 1
  }
done
mkdir -p "$OUTPUT_DIR/public"
OUTPUT_DIR=$(realpath "$OUTPUT_DIR")
case "$OUTPUT_DIR" in
  "$RUN_DIR"|"$RUN_DIR"/*) echo "evidence directory cannot be inside the disposable work directory" >&2; exit 1 ;;
esac

cd "$ROOT"
cargo build --release -p proofgate

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
    >"$OUTPUT_DIR/sequencer.log" 2>&1
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
  echo "standalone sequencer did not become healthy" >&2
  exit 1
fi

cd "$ROOT"
NETWORK_ROOT=$(target/release/proofgate sequencer-root \
  --sequencer-url "$SEQUENCER_URL")

WORK_DIR="$RUN_DIR/demo" \
RUN_SEQUENCER=1 \
RUN_MESSAGING=0 \
RUN_COMPOSITION=1 \
SEQUENCER_URL="$SEQUENCER_URL" \
ON_CHAIN_MAX_PROOF_AGE_MS=21600000 \
  scripts/demo.sh 2>&1 | tee "$OUTPUT_DIR/lifecycle.log"

for artifact in \
  proof.json \
  initial-gate-state.json \
  gate-state.json \
  access-badge.json \
  lez-proof.bin; do
  source_path="$RUN_DIR/demo/$artifact"
  [[ -s "$source_path" ]] || {
    printf 'missing public lifecycle artifact: %s\n' "$source_path" >&2
    exit 1
  }
  cp "$source_path" "$OUTPUT_DIR/public/$artifact"
done

mapfile -t transaction_hashes < <(
  sed -n 's/^Transaction hash: //p' "$OUTPUT_DIR/lifecycle.log"
)
mapfile -t account_ids < <(
  sed -n 's/^Account ID hex: //p' "$OUTPUT_DIR/lifecycle.log"
)
[[ "${#transaction_hashes[@]}" -eq 3 ]] || {
  echo "expected deployment, initialization, and claim transaction hashes" >&2
  exit 1
}
[[ "${#account_ids[@]}" -eq 2 ]] || {
  echo "expected gate and badge account IDs" >&2
  exit 1
}

STATE="$OUTPUT_DIR/public/gate-state.json"
INITIAL_STATE="$OUTPUT_DIR/public/initial-gate-state.json"
BADGE="$OUTPUT_DIR/public/access-badge.json"
PROOF="$OUTPUT_DIR/public/proof.json"
PERSISTENCE_STATE="$OUTPUT_DIR/public/persistence-gate-state.json"

printf "Restarting sequencer to validate persisted transactions and state\n"
kill "$SEQUENCER_PID"
wait "$SEQUENCER_PID" || true
SEQUENCER_PID=""
(
  cd "$RUN_DIR"
  exec "$SEQUENCER_BIN" "$SEQUENCER_CONFIG" \
    >"$OUTPUT_DIR/persistence-sequencer.log" 2>&1
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
  echo "restarted standalone sequencer did not become healthy" >&2
  exit 1
fi

for transaction_hash in "${transaction_hashes[@]}"; do
  payload=$(jq -cn --arg hash "$transaction_hash" \
    '{jsonrpc:"2.0",id:1,method:"getTransaction",params:[$hash]}')
  response=$(curl --fail --silent --show-error \
    --header "content-type: application/json" \
    --data "$payload" "$SEQUENCER_URL")
  jq -e '.result != null' <<<"$response" >/dev/null || {
    printf "transaction missing after restart: %s\n" "$transaction_hash" >&2
    exit 1
  }
done

target/release/proofgate on-chain fetch-state \
  --sequencer-url "$SEQUENCER_URL" \
  --gate-account-id-hex "${account_ids[0]}" \
  --output "$PERSISTENCE_STATE"
cmp -s "$STATE" "$PERSISTENCE_STATE" || {
  echo "GateState changed after sequencer restart" >&2
  exit 1
}

log_value() {
  local label=$1
  local value

  value=$(sed -n "s/^${label}: //p" "$OUTPUT_DIR/lifecycle.log" | tail -n 1)
  [[ -n "$value" ]] || {
    printf 'missing lifecycle metric: %s\n' "$label" >&2
    return 1
  }
  printf '%s\n' "$value"
}

{
  printf 'validated_at_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf 'sequencer_binary_sha256=%s\n' "$(sha256sum "$SEQUENCER_BIN" | cut -d ' ' -f 1)"
  printf "program_id=%s\n" "$(target/release/proofgate on-chain program-id)"
  printf "proofgate_producing_binary_sha256=%s\n" \
    "$(sha256sum target/release/proofgate | cut -d " " -f 1)"
  printf 'network_root=%s\n' "$NETWORK_ROOT"
  printf 'fixture_root=%s\n' "$(jq -r '.journal.commitment_root' "$PROOF")"
  printf 'gate_account=%s\n' "${account_ids[0]}"
  printf "private_badge_account=%s\n" "${account_ids[1]}"
  printf 'deployment_transaction=%s\n' "${transaction_hashes[0]}"
  printf 'initialization_transaction=%s\n' "${transaction_hashes[1]}"
  printf "claim_transaction=%s\n" "${transaction_hashes[2]}"
  printf "transactions_queryable_after_restart=%s\n" "${#transaction_hashes[@]}"
  printf 'gate_state_version=%s\n' "$(jq -r '.version' "$STATE")"
  printf 'max_proof_age_ms=%s\n' "$(jq -r '.max_proof_age_ms' "$STATE")"
  printf 'initial_claim_counter=%s\n' "$(jq -r '.claim_counter' "$INITIAL_STATE")"
  printf "final_claim_counter=%s\n" "$(jq -r ".claim_counter" "$STATE")"
  printf "stale_claim_denied=true\n"
  printf "persistent_state_after_restart=true\n"
  printf 'badge_claim_number=%s\n' "$(jq -r '.claim_number' "$BADGE")"
  printf "proof_issued_at_unix_ms=%s\n" "$(jq -r ".journal.issued_at_unix_ms" "$PROOF")"
  printf "balance_receipt_bytes=%s\n" \
    "$(jq -r ".receipt" "$PROOF" | base64 --decode | wc -c)"
  printf 'lez_proof_bytes=%s\n' "$(stat -c %s "$OUTPUT_DIR/public/lez-proof.bin")"
  printf 'public_post_states=%s\n' "$(log_value "Public post states")"
  printf 'encrypted_private_post_states=%s\n' "$(log_value "Encrypted private post states")"
  printf 'private_commitments=%s\n' "$(log_value "Private commitments")"
  printf 'nullifiers=%s\n' "$(log_value "Nullifiers")"
  printf 'gate_proving_ms=%s\n' "$(log_value "Gate proving time" | sed 's/ ms$//')"
  printf 'outer_proving_ms=%s\n' "$(log_value "LEZ PPE proving time" | sed 's/ ms$//')"
  printf 'total_composition_ms=%s\n' "$(log_value "Total composition time" | sed 's/ ms$//')"
  printf 'gate_total_cycles=%s\n' "$(log_value "Gate total cycles")"
  printf 'gate_user_cycles=%s\n' "$(log_value "Gate user cycles")"
  printf 'gate_paging_cycles=%s\n' "$(log_value "Gate paging cycles")"
  printf 'gate_segments=%s\n' "$(log_value "Gate segments")"
  printf 'outer_total_cycles=%s\n' "$(log_value "LEZ PPE total cycles")"
  printf 'outer_user_cycles=%s\n' "$(log_value "LEZ PPE user cycles")"
  printf 'outer_paging_cycles=%s\n' "$(log_value "LEZ PPE paging cycles")"
  printf 'outer_segments=%s\n' "$(log_value "LEZ PPE segments")"
} >"$OUTPUT_DIR/RESULTS.txt"

(
  cd "$OUTPUT_DIR"
  find . -type f ! -name SHA256SUMS -print0 |
    sort -z |
    xargs -0 sha256sum >SHA256SUMS
)

printf 'Standalone full claim lifecycle passed\n'
printf 'Evidence: %s\n' "$OUTPUT_DIR"

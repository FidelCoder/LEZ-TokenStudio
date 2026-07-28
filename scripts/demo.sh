#!/usr/bin/env bash
set -euo pipefail
umask 077

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
WORK_DIR="${WORK_DIR:-$(mktemp -d /tmp/proofgate-demo-XXXXXX)}"
PROOFGATE_BIN="${PROOFGATE_BIN:-$ROOT/target/release/proofgate}"
RUN_COMPOSITION="${RUN_COMPOSITION:-1}"
RUN_MESSAGING="${RUN_MESSAGING:-0}"
RUN_SEQUENCER="${RUN_SEQUENCER:-0}"
DEPLOY_GATE="${DEPLOY_GATE:-1}"
ON_CHAIN_MAX_PROOF_AGE_MS="${ON_CHAIN_MAX_PROOF_AGE_MS:-21600000}"
EXISTING_PROOF="${EXISTING_PROOF:-}"
EXISTING_PRESENTER_KEY="${EXISTING_PRESENTER_KEY:-}"

TOKEN="$WORK_DIR/token.json"
GATE="$WORK_DIR/gate.json"
PRESENTER_KEY="$WORK_DIR/presenter.json"
WITNESS="$WORK_DIR/witness.json"
LOW_WITNESS="$WORK_DIR/insufficient-witness.json"
PROOF="$WORK_DIR/proof.json"
CHALLENGE="$WORK_DIR/challenge.json"
ENVELOPE="$WORK_DIR/envelope.json"
REPLAY_CACHE="$WORK_DIR/replay-cache.json"
STATE="$WORK_DIR/gate-state.json"
CLAIM="$WORK_DIR/claim.json"
LEZ_PROOF="$WORK_DIR/lez-proof.bin"
INITIAL_STATE="$WORK_DIR/initial-gate-state.json"
BADGE="$WORK_DIR/access-badge.json"
GATE_ACCOUNT_KEY="$WORK_DIR/gate-account.json"
BADGE_ACCOUNT_KEY="$WORK_DIR/badge-account.json"

TOKEN_OWNER_HEX="${TOKEN_OWNER_HEX:-000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f}"
TOKEN_DEFINITION_HEX="${TOKEN_DEFINITION_HEX:-1111111111111111111111111111111111111111111111111111111111111111}"
GATE_ACCOUNT_HEX="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
BADGE_ACCOUNT_HEX="bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
CHALLENGE_NONCE_HEX="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
FIXTURE_COMMITMENT_ROOT_HEX="${FIXTURE_COMMITMENT_ROOT_HEX:-510dbd3aa09bae25bc9e683e65ff2104eba32565f9c10b91582596955505b778}"

command -v cargo >/dev/null || { echo "cargo is required" >&2; exit 1; }
command -v jq >/dev/null || { echo "jq is required" >&2; exit 1; }
mkdir -p "$WORK_DIR"
if [[ -n "$EXISTING_PROOF" || -n "$EXISTING_PRESENTER_KEY" ]]; then
  [[ -n "$EXISTING_PROOF" && -n "$EXISTING_PRESENTER_KEY" ]] || {
    echo "EXISTING_PROOF and EXISTING_PRESENTER_KEY must be set together" >&2
    exit 1
  }
  [[ -s "$EXISTING_PROOF" && -s "$EXISTING_PRESENTER_KEY" ]] || {
    echo "existing proof or presenter key is missing or empty" >&2
    exit 1
  }
fi

wait_for_transaction() {
  local hash=$1
  local attempts=${2:-120}
  local payload
  local response

  payload=$(jq -cn --arg hash "$hash" \
    '{jsonrpc:"2.0",id:1,method:"getTransaction",params:[$hash]}')
  for _ in $(seq 1 "$attempts"); do
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

echo "[1/11] Build ProofGate"
cargo build --manifest-path "$ROOT/Cargo.toml" --release -p proofgate

echo "[2/11] Preview LEZ token creation"
"$PROOFGATE_BIN" token create \
  --name "Founders Token" \
  --symbol FNDR \
  --decimals 0 \
  --definition-account-id-hex "$TOKEN_DEFINITION_HEX" \
  --token-program-owner-hex "$TOKEN_OWNER_HEX" \
  --definition-account Private/founders-definition \
  --supply-account Private/founders-supply \
  --issuer-account Private/founders-issuer \
  --total-supply 1000000 \
  --output "$TOKEN" \
  --wallet-binary "${WALLET_BIN:-wallet}" \
  --dry-run

echo "[3/11] Configure the exact token threshold gate"
"$PROOFGATE_BIN" gate init \
  --token "$TOKEN" \
  --application-id tokenstudio \
  --gate-id founders-chat \
  --threshold 100 \
  --verifier-id logos-chat:founders \
  --output "$GATE"

echo "[4/11] Create presenter identity and deterministic private witnesses"
if [[ -n "$EXISTING_PRESENTER_KEY" ]]; then
  cp "$EXISTING_PRESENTER_KEY" "$PRESENTER_KEY"
  chmod 600 "$PRESENTER_KEY"
  echo "Reused matching presenter identity"
else
  "$PROOFGATE_BIN" presenter generate --output "$PRESENTER_KEY"
fi
PRESENTER_PUBLIC_KEY=$("$PROOFGATE_BIN" presenter public-key --key "$PRESENTER_KEY")
cargo run --quiet --release \
  --manifest-path "$ROOT/Cargo.toml" \
  -p attestation-prover --example generate_demo_input -- "$WITNESS" 250
cargo run --quiet --release \
  --manifest-path "$ROOT/Cargo.toml" \
  -p attestation-prover --example generate_demo_input -- "$LOW_WITNESS" 99

echo "[5/11] Generate real private balance proof (RISC0_DEV_MODE=0)"
if [[ -n "$EXISTING_PROOF" ]]; then
  cp "$EXISTING_PROOF" "$PROOF"
  echo "Reused existing real private balance proof"
elif [[ -n "${ACCOUNT_SNAPSHOT:-}" && -n "${SEQUENCER_URL:-}" ]]; then
  RISC0_DEV_MODE=0 "$PROOFGATE_BIN" prove \
    --gate "$GATE" \
    --account-snapshot "$ACCOUNT_SNAPSHOT" \
    --sequencer-url "$SEQUENCER_URL" \
    --presenter-public-key-hex "$PRESENTER_PUBLIC_KEY" \
    --output "$PROOF"
else
  RISC0_DEV_MODE=0 "$PROOFGATE_BIN" prove \
    --gate "$GATE" \
    --input "$WITNESS" \
    --presenter-public-key-hex "$PRESENTER_PUBLIC_KEY" \
    --output "$PROOF"
fi
PROOF_ROOT_HEX=$(jq -er '.journal.commitment_root' "$PROOF")
if [[ "$RUN_SEQUENCER" == "1" && -n "${ACCOUNT_SNAPSHOT:-}" ]]; then
  COMMITMENT_ROOT_HEX=$("$PROOFGATE_BIN" sequencer-root \
    --sequencer-url "$SEQUENCER_URL")
  test "$COMMITMENT_ROOT_HEX" = "$PROOF_ROOT_HEX" || {
    echo "live proof root does not match the independently queried sequencer root" >&2
    exit 1
  }
else
  test "$PROOF_ROOT_HEX" = "$FIXTURE_COMMITMENT_ROOT_HEX" || {
    echo "fixture proof does not match the verifier-pinned fixture root" >&2
    exit 1
  }
  COMMITMENT_ROOT_HEX="$FIXTURE_COMMITMENT_ROOT_HEX"
  [[ "$RUN_SEQUENCER" != "1" ]] || echo "Using explicit fixture root for local claim lifecycle"
fi

echo "[6/11] Issue challenge, present proof, and verify locally"
LOCAL_TIME_ARGS=()
if [[ -n "$EXISTING_PROOF" ]]; then
  LOCAL_VERIFICATION_TIME=$(jq -er '.journal.issued_at_unix_ms' "$PROOF")
  LOCAL_TIME_ARGS=(--now-unix-ms "$LOCAL_VERIFICATION_TIME")
  echo "Replaying the existing proof at its original verification time"
fi
"$PROOFGATE_BIN" challenge create \
  --gate "$GATE" \
  --ttl-ms 300000 \
  --commitment-root-hex "$COMMITMENT_ROOT_HEX" \
  "${LOCAL_TIME_ARGS[@]}" \
  --output "$CHALLENGE"
"$PROOFGATE_BIN" present \
  --proof "$PROOF" \
  --challenge "$CHALLENGE" \
  --presenter-key "$PRESENTER_KEY" \
  --transport local \
  --output "$ENVELOPE"
"$PROOFGATE_BIN" verify \
  --gate "$GATE" \
  --envelope "$ENVELOPE" \
  --challenge "$CHALLENGE" \
  --replay-cache "$REPLAY_CACHE" \
  "${LOCAL_TIME_ARGS[@]}"

echo "[7/11] Confirm persistent replay denial"
set +e
REPLAY_OUTPUT=$("$PROOFGATE_BIN" verify \
  --gate "$GATE" \
  --envelope "$ENVELOPE" \
  --challenge "$CHALLENGE" \
  --replay-cache "$REPLAY_CACHE" \
  "${LOCAL_TIME_ARGS[@]}" 2>&1)
REPLAY_STATUS=$?
set -e
if [[ $REPLAY_STATUS -eq 0 ]] || ! grep -Fq '[1010]' <<<"$REPLAY_OUTPUT"; then
  echo "expected replay denial [1010], got: $REPLAY_OUTPUT" >&2
  exit 1
fi
echo "Replay denied [1010]"

echo "[8/11] Confirm insufficient private balance cannot produce a proof"
set +e
LOW_OUTPUT=$(RISC0_DEV_MODE=0 "$PROOFGATE_BIN" prove \
  --gate "$GATE" \
  --input "$LOW_WITNESS" \
  --presenter-public-key-hex "$PRESENTER_PUBLIC_KEY" \
  --output "$WORK_DIR/insufficient-proof.json" 2>&1)
LOW_STATUS=$?
set -e
if [[ $LOW_STATUS -eq 0 ]]; then
  echo "insufficient balance unexpectedly produced a proof" >&2
  exit 1
fi
echo "Insufficient balance denied"

echo "[9/11] Execute the actual SPEL access-badge transition"
if [[ "$RUN_SEQUENCER" == "1" ]]; then
  : "${SEQUENCER_URL:?set SEQUENCER_URL when RUN_SEQUENCER=1}"
  GATE_KEY_OUTPUT=$("$PROOFGATE_BIN" on-chain account-generate \
    --output "$GATE_ACCOUNT_KEY")
  printf '%s\n' "$GATE_KEY_OUTPUT"
  GATE_ACCOUNT_HEX=$(sed -n 's/^Account ID hex: //p' <<<"$GATE_KEY_OUTPUT")
  test "${#GATE_ACCOUNT_HEX}" -eq 64 || {
    echo "gate account generation did not return a 32-byte hex ID" >&2
    exit 1
  }
  if [[ "$DEPLOY_GATE" == "1" ]]; then
    DEPLOYMENT_OUTPUT=$("$PROOFGATE_BIN" on-chain deploy --sequencer-url "$SEQUENCER_URL")
    printf '%s\n' "$DEPLOYMENT_OUTPUT"
    DEPLOYMENT_TX=$(sed -n 's/^Transaction hash: //p' <<<"$DEPLOYMENT_OUTPUT")
    test -n "$DEPLOYMENT_TX"
    wait_for_transaction "$DEPLOYMENT_TX"
  fi
fi

if [[ "$RUN_SEQUENCER" == "1" || "$RUN_COMPOSITION" == "1" ]]; then
  BADGE_KEY_OUTPUT=$("$PROOFGATE_BIN" on-chain private-account-generate \
    --output "$BADGE_ACCOUNT_KEY")
  printf '%s\n' "$BADGE_KEY_OUTPUT"
  BADGE_ACCOUNT_HEX=$(sed -n 's/^Account ID hex: //p' <<<"$BADGE_KEY_OUTPUT")
  test "${#BADGE_ACCOUNT_HEX}" -eq 64 || {
    echo "private badge generation did not return a 32-byte hex ID" >&2
    exit 1
  }
fi

"$PROOFGATE_BIN" on-chain init \
  --gate "$GATE" \
  --challenge-nonce-hex "$CHALLENGE_NONCE_HEX" \
  --commitment-root-hex "$COMMITMENT_ROOT_HEX" \
  --max-proof-age-ms "$ON_CHAIN_MAX_PROOF_AGE_MS" \
  --output "$STATE"
cp "$STATE" "$INITIAL_STATE"
if [[ "$RUN_SEQUENCER" == "1" ]]; then
  INITIALIZATION_OUTPUT=$("$PROOFGATE_BIN" on-chain initialize-submit \
    --sequencer-url "$SEQUENCER_URL" \
    --state "$STATE" \
    --gate-key "$GATE_ACCOUNT_KEY")
  printf '%s\n' "$INITIALIZATION_OUTPUT"
  INITIALIZATION_TX=$(sed -n 's/^Transaction hash: //p' <<<"$INITIALIZATION_OUTPUT")
  test -n "$INITIALIZATION_TX"
  wait_for_transaction "$INITIALIZATION_TX"
  "$PROOFGATE_BIN" on-chain fetch-state \
    --sequencer-url "$SEQUENCER_URL" \
    --gate-account-id-hex "$GATE_ACCOUNT_HEX" \
    --output "$STATE"
fi
"$PROOFGATE_BIN" on-chain present \
  --proof "$PROOF" \
  --state "$STATE" \
  --claim-account-id-hex "$BADGE_ACCOUNT_HEX" \
  --presenter-key "$PRESENTER_KEY" \
  --output "$CLAIM"
"$PROOFGATE_BIN" on-chain simulate \
  --state "$STATE" \
  --claim "$CLAIM" \
  --gate-account-id-hex "$GATE_ACCOUNT_HEX" \
  --badge-account-id-hex "$BADGE_ACCOUNT_HEX"

echo "[10/11] Compose official LEZ private-execution proof"
if [[ "$RUN_SEQUENCER" == "1" ]]; then
  CLAIM_OUTPUT=$(RISC0_DEV_MODE=0 "$PROOFGATE_BIN" on-chain claim-submit \
    --sequencer-url "$SEQUENCER_URL" \
    --proof "$PROOF" \
    --state "$STATE" \
    --claim "$CLAIM" \
    --gate-account-id-hex "$GATE_ACCOUNT_HEX" \
    --badge-key "$BADGE_ACCOUNT_KEY" \
    --output-badge "$BADGE" \
    --output-lez-proof "$LEZ_PROOF")
  printf '%s\n' "$CLAIM_OUTPUT"
  CLAIM_TX=$(sed -n 's/^Transaction hash: //p' <<<"$CLAIM_OUTPUT")
  test -n "$CLAIM_TX"
  wait_for_transaction "$CLAIM_TX" 600
  set +e
  ON_CHAIN_REPLAY_OUTPUT=$(RISC0_DEV_MODE=0 "$PROOFGATE_BIN" on-chain claim-submit \
    --sequencer-url "$SEQUENCER_URL" \
    --proof "$PROOF" \
    --state "$STATE" \
    --claim "$CLAIM" \
    --gate-account-id-hex "$GATE_ACCOUNT_HEX" \
    --badge-key "$BADGE_ACCOUNT_KEY" \
    --output-badge "$WORK_DIR/replayed-access-badge.json" \
    --output-lez-proof "$WORK_DIR/replayed-lez-proof.bin" 2>&1)
  ON_CHAIN_REPLAY_STATUS=$?
  set -e
  if [[ $ON_CHAIN_REPLAY_STATUS -eq 0 ]] \
    || ! grep -Fq 'gate state changed after the challenge was issued' <<<"$ON_CHAIN_REPLAY_OUTPUT"; then
    echo "expected stale on-chain claim rejection, got: $ON_CHAIN_REPLAY_OUTPUT" >&2
    exit 1
  fi
  echo "Stale on-chain claim denied"
  "$PROOFGATE_BIN" on-chain fetch-state \
    --sequencer-url "$SEQUENCER_URL" \
    --gate-account-id-hex "$GATE_ACCOUNT_HEX" \
    --output "$STATE"
  test "$(jq -r '.version' "$STATE")" = "3"
  test "$(jq -r '.claim_counter' "$STATE")" = "1"
  test "$(jq -r '.max_proof_age_ms' "$STATE")" = "$ON_CHAIN_MAX_PROOF_AGE_MS"
  test "$(jq -c '.challenge_nonce' "$STATE")" != \
    "$(jq -c '.challenge_nonce' "$INITIAL_STATE")"
  test "$(jq -r '.claim_number' "$BADGE")" = "1"
  test "$(jq -c '.context_hash' "$BADGE")" = \
    "$(jq -c '.context_hash' "$STATE")"
  BADGE_PRESENTER_HEX=$(jq -r '.presenter_public_key[]' "$BADGE" |
    while IFS= read -r byte; do
      printf '%02x' "$byte"
    done)
  test "$BADGE_PRESENTER_HEX" = \
    "$(jq -r '.journal.presenter_public_key' "$PROOF")"
  test "$(jq -r '.proof_issued_at_unix_ms' "$BADGE")" = \
    "$(jq -r '.journal.issued_at_unix_ms' "$PROOF")"
  echo "On-chain state rotation and access badge assertions passed"
elif [[ "$RUN_COMPOSITION" == "1" ]]; then
  RISC0_DEV_MODE=0 "$PROOFGATE_BIN" on-chain compose \
    --proof "$PROOF" \
    --state "$STATE" \
    --claim "$CLAIM" \
    --gate-account-id-hex "$GATE_ACCOUNT_HEX" \
    --badge-key "$BADGE_ACCOUNT_KEY" \
    --output-badge "$BADGE" \
    --output-lez-proof "$LEZ_PROOF"
else
  echo "Skipped by RUN_COMPOSITION=$RUN_COMPOSITION"
fi

echo "[11/11] Run encrypted token-gated chat flow"
if [[ "$RUN_MESSAGING" == "1" ]]; then
  GATE="$GATE" \
  PROOF="$PROOF" \
  PRESENTER_KEY="$PRESENTER_KEY" \
  PROOFGATE_BIN="$PROOFGATE_BIN" \
  WORK_DIR="$WORK_DIR/chat" \
    "$ROOT/demos/token-gated-chat/run.sh"
else
  echo "Skipped by RUN_MESSAGING=$RUN_MESSAGING"
fi

echo "PASS: token setup, private proof, local verification, negative cases, SPEL gate"
echo "Artifacts: $WORK_DIR"

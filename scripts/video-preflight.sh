#!/usr/bin/env bash
set -euo pipefail
umask 077

if [[ $# -gt 2 ]]; then
  printf 'usage: %s [private-proof-bundle] [output-dir]\n' "$0" >&2
  exit 2
fi

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
PRIVATE_BUNDLE=${1:-/tmp/proofgate-testnet-claim-current}
STARTED_AT=$(date -u +%Y%m%dT%H%M%SZ)
OUTPUT_DIR=${2:-"$ROOT/artifacts/video-preflight-$STARTED_AT"}
PUBLIC_BUNDLE="$ROOT/artifacts/testnet-claim-current"
LOCAL_BUNDLE="$ROOT/artifacts/local-submission-final"
CHAT_BUNDLE="$ROOT/artifacts/live-chat-current"
BASECAMP_BUNDLE="$ROOT/artifacts/basecamp-current"
PROOFGATE_BIN=${PROOFGATE_BIN:-"$ROOT/target/release/proofgate"}

GATE_ACCOUNT_ID=4f7b5ec8898c8509b4b18f39c646a838d37b6c00e7ffe251c410b91645640eb0
BADGE_ACCOUNT_ID=666e14ec27217227692569b0557b86c4edc6f4ffe8a6d82f7d668cd23310ee99
INITIALIZATION_TX=5a4f7b7e5ceccadfd46d42f841dfff75d28b14458ea7a796cff6c0e52f164978
CLAIM_TX=8f34ef536703cb5f3b2401d789dcdcbf3a0f28b4b6be1ebf6a6711c62a1ef1de

require_command() {
  command -v "$1" >/dev/null || {
    printf '%s is required\n' "$1" >&2
    exit 1
  }
}

require_file() {
  [[ -s "$1" ]] || {
    printf 'missing required video artifact: %s\n' "$1" >&2
    exit 1
  }
}

for command in cmp curl grep jq sha256sum; do
  require_command "$command"
done
require_file "$PROOFGATE_BIN"
require_file "$PUBLIC_BUNDLE/public/balance-proof.json"
require_file "$PUBLIC_BUNDLE/public/claim.json"
require_file "$PUBLIC_BUNDLE/public/claim-submit.log"
require_file "$PUBLIC_BUNDLE/public/gate.json"
require_file "$PUBLIC_BUNDLE/public/initial-gate-state.json"
require_file "$CHAT_BUNDLE/transcript.log"
require_file "$BASECAMP_BUNDLE/integration/app-data/proofgate-desktop.png"

PRIVATE_MODE=0
if [[ -s "$PRIVATE_BUNDLE/proof.json" \
  && -s "$PRIVATE_BUNDLE/presenter-key.json" \
  && -s "$PRIVATE_BUNDLE/balance-prove.log" \
  && -s "$PRIVATE_BUNDLE/claim-submit.log" ]]; then
  PRIVATE_MODE=1
fi

if [[ -e "$OUTPUT_DIR" ]]; then
  printf 'output directory already exists; choose a fresh path: %s\n' "$OUTPUT_DIR" >&2
  exit 1
fi
mkdir -p "$OUTPUT_DIR/off-chain"

cd "$ROOT"

printf '\n[1/6] Verify preserved evidence integrity\n'
(cd "$PUBLIC_BUNDLE" && sha256sum --check SHA256SUMS >/dev/null)
(cd "$LOCAL_BUNDLE" && sha256sum --check SHA256SUMS >/dev/null)
(cd "$CHAT_BUNDLE" && sha256sum --check SHA256SUMS >/dev/null)
sha256sum --check "$BASECAMP_BUNDLE/SHA256SUMS" >/dev/null
if [[ $PRIVATE_MODE -eq 1 ]]; then
  cmp "$PRIVATE_BUNDLE/proof.json" "$PUBLIC_BUNDLE/public/balance-proof.json"
  grep -Fq 'RISC0_DEV_MODE: disabled (real proving)' \
    "$PRIVATE_BUNDLE/balance-prove.log"
  grep -Fq 'RISC0_DEV_MODE: disabled (real LEZ claim submission)' \
    "$PRIVATE_BUNDLE/claim-submit.log"
  printf 'PASS: checksummed bundles and matching real Risc0 receipt\n'
  grep -E 'RISC0_DEV_MODE:|Receipt bytes:|Proof bytes:|Total composition time:|Transaction hash:' \
    "$PRIVATE_BUNDLE/balance-prove.log" "$PRIVATE_BUNDLE/claim-submit.log"
else
  grep -Fq 'RISC0_DEV_MODE: disabled (real LEZ claim submission)' \
    "$PUBLIC_BUNDLE/public/claim-submit.log"
  printf 'PASS: checksummed public real-proof and claim bundles\n'
  printf 'Mode: public evidence (temporary presenter key is unavailable)\n'
  grep -E 'RISC0_DEV_MODE:|Proof bytes:|Total composition time:|Transaction hash:' \
    "$PUBLIC_BUNDLE/public/claim-submit.log"
fi
jq '{threshold:.journal.threshold, commitment_root:.journal.commitment_root,
  context_hash:.journal.context_hash, presenter_public_key:.journal.presenter_public_key,
  issued_at_unix_ms:.journal.issued_at_unix_ms}' \
  "$PUBLIC_BUNDLE/public/balance-proof.json"

printf '\n[2/6] Re-verify the real receipt and presenter binding off-chain\n'
if [[ $PRIVATE_MODE -eq 1 ]]; then
  PROOF_ISSUED_AT=$(jq -er '.journal.issued_at_unix_ms' \
    "$PUBLIC_BUNDLE/public/balance-proof.json")
  PROOF_ROOT=$(jq -er '.journal.commitment_root' \
    "$PUBLIC_BUNDLE/public/balance-proof.json")
  "$PROOFGATE_BIN" challenge create \
    --gate "$PUBLIC_BUNDLE/public/gate.json" \
    --commitment-root-hex "$PROOF_ROOT" \
    --ttl-ms 300000 \
    --now-unix-ms "$PROOF_ISSUED_AT" \
    --output "$OUTPUT_DIR/off-chain/challenge.json"
  "$PROOFGATE_BIN" present \
    --proof "$PRIVATE_BUNDLE/proof.json" \
    --challenge "$OUTPUT_DIR/off-chain/challenge.json" \
    --presenter-key "$PRIVATE_BUNDLE/presenter-key.json" \
    --transport local \
    --output "$OUTPUT_DIR/off-chain/envelope.json"
  "$PROOFGATE_BIN" verify \
    --gate "$PUBLIC_BUNDLE/public/gate.json" \
    --envelope "$OUTPUT_DIR/off-chain/envelope.json" \
    --challenge "$OUTPUT_DIR/off-chain/challenge.json" \
    --replay-cache "$OUTPUT_DIR/off-chain/replay-cache.json" \
    --now-unix-ms "$PROOF_ISSUED_AT"

  set +e
  REPLAY_OUTPUT=$("$PROOFGATE_BIN" verify \
    --gate "$PUBLIC_BUNDLE/public/gate.json" \
    --envelope "$OUTPUT_DIR/off-chain/envelope.json" \
    --challenge "$OUTPUT_DIR/off-chain/challenge.json" \
    --replay-cache "$OUTPUT_DIR/off-chain/replay-cache.json" \
    --now-unix-ms "$PROOF_ISSUED_AT" 2>&1)
  REPLAY_STATUS=$?
  set -e
  if [[ $REPLAY_STATUS -eq 0 ]] || ! grep -Fq '[1010]' <<<"$REPLAY_OUTPUT"; then
    printf 'expected replay denial [1010], got: %s\n' "$REPLAY_OUTPUT" >&2
    exit 1
  fi
  printf 'PASS: replay denied [1010]\n'
else
  printf 'SKIP: presenter re-signing requires the deleted temporary private key\n'
  printf 'PASS: checksummed Messaging verification and replay evidence follows\n'
fi

printf '\n[3/6] Execute the actual SPEL on-chain verifier locally\n'
"$PROOFGATE_BIN" on-chain simulate \
  --state "$PUBLIC_BUNDLE/public/initial-gate-state.json" \
  --claim "$PUBLIC_BUNDLE/public/claim.json" \
  --gate-account-id-hex "$GATE_ACCOUNT_ID" \
  --badge-account-id-hex "$BADGE_ACCOUNT_ID"

printf '\n[4/6] Re-fetch the included claim from the official LEZ testnet\n'
"$ROOT/scripts/ci-testnet-evidence.sh" \
  "$OUTPUT_DIR/testnet" \
  "$GATE_ACCOUNT_ID" \
  "$INITIALIZATION_TX" \
  "$CLAIM_TX"
cat "$OUTPUT_DIR/testnet/summary.txt"

printf '\n[5/6] Verify and display the sanitized Logos Messaging flow\n'
for marker in \
  'Challenge sent over Logos Messaging' \
  'Forwarded presentation denied [1008]' \
  'ALLOW (Logos Messaging)' \
  'Member admitted:' \
  'replay denial'; do
  grep -Fq "$marker" "$CHAT_BUNDLE/transcript.log" || {
    printf 'missing Messaging evidence marker: %s\n' "$marker" >&2
    exit 1
  }
done
cat "$CHAT_BUNDLE/transcript.log"

printf '\n[6/6] Confirm the Basecamp package and rendered UI\n'
printf 'LGX: %s\n' "$BASECAMP_BUNDLE/logos-tokenstudio_proofgate_ui-module.lgx"
printf 'Screenshot: %s\n' \
  "$BASECAMP_BUNDLE/integration/app-data/proofgate-desktop.png"

printf '\nVIDEO PREFLIGHT PASS\n'
printf 'Generated evidence: %s\n' "$OUTPUT_DIR"
printf 'No private key or wallet contents were printed.\n'

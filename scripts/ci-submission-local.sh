#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 2 || $# -gt 3 ]]; then
  printf 'usage: %s <sequencer-binary> <sequencer-config> [evidence-dir]\n' "$0" >&2
  exit 2
fi

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
SEQUENCER_BIN=$(realpath "$1")
SEQUENCER_CONFIG=$(realpath "$2")
STARTED_AT=$(date -u +%Y%m%dT%H%M%SZ)
EVIDENCE_DIR=${3:-"$ROOT/artifacts/local-submission-$STARTED_AT"}

mkdir -p "$EVIDENCE_DIR/logs"
EVIDENCE_DIR=$(realpath "$EVIDENCE_DIR")

(
  cd "$ROOT"
  git ls-files --cached --others --exclude-standard -z |
    sort -z |
    xargs -0 sha256sum >"$EVIDENCE_DIR/SOURCE_SHA256SUMS"
)

run_logged() {
  local name=$1
  shift

  printf '\n==> %s\n' "$name"
  "$@" 2>&1 | tee "$EVIDENCE_DIR/logs/$name.log"
}

{
  printf 'started_at_utc=%s\n' "$STARTED_AT"
  printf 'repository=%s\n' "$ROOT"
  printf 'head=%s\n' "$(git -C "$ROOT" rev-parse HEAD)"
  printf 'rust=%s\n' "$(rustc --version)"
  printf 'cargo=%s\n' "$(cargo --version)"
  printf 'docker=%s\n' "$(docker --version)"
  printf 'basecamp_flake_lock_sha256=%s\n' \
    "$(sha256sum "$ROOT/apps/basecamp-tokenstudio/flake.lock" | cut -d ' ' -f 1)"
  printf 'sequencer_binary=%s\n' "$SEQUENCER_BIN"
  printf 'sequencer_binary_sha256=%s\n' "$(sha256sum "$SEQUENCER_BIN" | cut -d ' ' -f 1)"
  printf 'sequencer_config=%s\n' "$SEQUENCER_CONFIG"
  printf 'worktree_status_begin\n'
  git -C "$ROOT" status --short
  printf 'worktree_status_end\n'
} >"$EVIDENCE_DIR/environment.txt"

run_logged rust-ci "$ROOT/scripts/ci-local.sh"
run_logged release-build cargo build --manifest-path "$ROOT/Cargo.toml" --release \
  -p proofgate -p proofgate-governance
run_logged standalone-lez \
  "$ROOT/scripts/ci-standalone-lez.sh" "$SEQUENCER_BIN" "$SEQUENCER_CONFIG"
run_logged standalone-claim \
  "$ROOT/scripts/ci-standalone-claim.sh" "$SEQUENCER_BIN" "$SEQUENCER_CONFIG" \
  "$EVIDENCE_DIR/standalone-claim"
run_logged basecamp "$ROOT/scripts/basecamp-local.sh" "$EVIDENCE_DIR/basecamp"

FINISHED_AT=$(date -u +%Y%m%dT%H%M%SZ)
printf 'finished_at_utc=%s\n' "$FINISHED_AT" >>"$EVIDENCE_DIR/environment.txt"

(
  cd "$EVIDENCE_DIR"
  find . -type f ! -name SHA256SUMS -print0 |
    sort -z |
    xargs -0 sha256sum >SHA256SUMS
)

printf '\nLocal submission CI passed\n'
printf 'Evidence: %s\n' "$EVIDENCE_DIR"
printf 'Checksums: %s\n' "$EVIDENCE_DIR/SHA256SUMS"

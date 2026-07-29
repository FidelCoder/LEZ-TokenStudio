#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"

run_step() {
  local name=$1
  shift
  printf '\n==> %s\n' "$name"
  "$@"
}

run_step "Check local CI shell syntax" bash -n scripts/ci-local.sh
run_step "Check standalone LEZ CI shell syntax" bash -n scripts/ci-standalone-lez.sh
run_step "Check standalone claim CI shell syntax" bash -n scripts/ci-standalone-claim.sh
run_step "Check local submission CI shell syntax" bash -n scripts/ci-submission-local.sh
run_step "Check testnet evidence shell syntax" bash -n scripts/ci-testnet-evidence.sh
run_step "Check local Basecamp runner shell syntax" bash -n scripts/basecamp-local.sh
run_step "Check video preflight shell syntax" bash -n scripts/video-preflight.sh
run_step "Check demo shell syntax" bash -n scripts/demo.sh
run_step "Check Messaging demo shell syntax" bash -n demos/token-gated-chat/run.sh
run_step "Check Basecamp UI test syntax" node --check apps/basecamp-tokenstudio/tests/ui-tests.mjs
run_step "Check Basecamp command contract syntax" \
  node --check scripts/check-basecamp-command-contract.mjs
run_step "Validate Basecamp private badge command contract" \
  node scripts/check-basecamp-command-contract.mjs
run_step "Validate Basecamp metadata" jq -e . apps/basecamp-tokenstudio/metadata.json
run_step "Validate committed SPEL IDL" jq -e . programs/balance-gate/idl/balance_gate.idl.json

printf '\n==> Compare generated and committed SPEL IDL\n'
generated=$(cargo run --quiet -p balance-gate-methods --example generate_idl | jq -S -c .)
committed=$(jq -S -c . programs/balance-gate/idl/balance_gate.idl.json)
if [[ "$generated" != "$committed" ]]; then
  printf '%s\n' "generated SPEL IDL differs from the committed artifact" >&2
  exit 1
fi

run_step "Check Rust formatting" cargo fmt --all --check
run_step "Run strict Clippy" cargo clippy --workspace --all-targets -- -D warnings
run_step "Run workspace tests" cargo test --workspace
run_step "Run LEZ private badge circuit shape regression" \
  env RISC0_DEV_MODE=1 cargo test -p lez-gate-sdk \
  private_badge_execution_has_a_sequencer_acceptable_shape -- --ignored

printf '\nLocal CI passed\n'
printf 'Commit: %s\n' "$(git rev-parse HEAD)"
printf 'Rust: %s\n' "$(rustc --version)"
printf 'Cargo: %s\n' "$(cargo --version)"

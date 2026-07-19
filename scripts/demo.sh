#!/usr/bin/env bash
set -euo pipefail

TOKEN_OWNER_HEX="000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"

cargo run -p proofgate -- context-hash \
  --application-id tokenstudio \
  --gate-id founders-chat \
  --token-owner-hex "$TOKEN_OWNER_HEX" \
  --threshold 100 \
  --verifier-id logos-chat:founders \
  --expires-at-unix-ms 1800000000000

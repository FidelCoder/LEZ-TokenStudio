#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
OUTPUT_DIR=${1:-"$ROOT/artifacts/basecamp-current"}
CONTAINER=${BASECAMP_NIX_CONTAINER:-proofgate-basecamp-local-v2}
IMAGE=${BASECAMP_NIX_IMAGE:-nixos/nix:2.20.6}
INTEGRATION_ATTR=${BASECAMP_INTEGRATION_ATTR:-integration-test}
NIX_CONFIG_VALUE="experimental-features = nix-command flakes"
NIX_MAX_JOBS=${BASECAMP_NIX_MAX_JOBS:-1}
NIX_CORES=${BASECAMP_NIX_CORES:-2}

command -v docker >/dev/null || {
  echo "docker is required for the pinned Basecamp build" >&2
  exit 1
}
mkdir -p "$OUTPUT_DIR"

if docker container inspect "$CONTAINER" >/dev/null 2>&1; then
  WORKSPACE_MOUNT=$(docker inspect --format '{{range .Mounts}}{{if eq .Destination "/workspace"}}{{.Source}}{{end}}{{end}}' "$CONTAINER")
  [[ -n "$WORKSPACE_MOUNT" ]] || {
    printf 'container %s has no /workspace mount\n' "$CONTAINER" >&2
    exit 1
  }
  WORKSPACE_MOUNT=$(realpath "$WORKSPACE_MOUNT")
  [[ "$WORKSPACE_MOUNT" == "$ROOT" ]] || {
    printf 'container %s mounts /workspace from %s, expected %s\n' \
      "$CONTAINER" "$WORKSPACE_MOUNT" "$ROOT" >&2
    exit 1
  }
  if [[ $(docker inspect --format '{{.State.Running}}' "$CONTAINER") != "true" ]]; then
    docker start "$CONTAINER" >/dev/null
  fi
else
  run_args=(
    --detach
    --name "$CONTAINER"
    --env "NIX_CONFIG=$NIX_CONFIG_VALUE"
    --ulimit nofile=1048576:1048576
    --volume "$ROOT:/workspace:ro"
  )
  docker run "${run_args[@]}" "$IMAGE" \
    sh -c 'trap "exit 0" TERM INT; while :; do sleep 3600; done' >/dev/null
fi
CONTAINER_IMAGE=$(docker inspect --format '{{.Config.Image}}' "$CONTAINER")
CONTAINER_IMAGE_ID=$(docker inspect --format '{{.Image}}' "$CONTAINER")

build_attr() {
  local attribute=$1
  local store_path
  local -a build_command

  build_command=(
    docker exec
    --env "NIX_CONFIG=$NIX_CONFIG_VALUE"
    "$CONTAINER"
    nix build "path:/workspace/apps/basecamp-tokenstudio#$attribute"
    --no-link
    --print-out-paths
    --max-jobs "$NIX_MAX_JOBS"
    --cores "$NIX_CORES"
  )
  store_path=$("${build_command[@]}")
  store_path=${store_path##*$'\n'}
  [[ "$store_path" == /nix/store/* ]] || {
    printf 'Nix did not return a store path for %s: %s\n' "$attribute" "$store_path" >&2
    exit 1
  }
  printf '%s\n' "$store_path"
}

printf 'Building current Basecamp LGX with %s in %s\n' "$CONTAINER_IMAGE" "$CONTAINER"
LGX_STORE=$(build_attr lgx)
LGX_FILE=$(docker exec "$CONTAINER" find "$LGX_STORE" \
  -type f -name '*.lgx' -print -quit)
[[ -n "$LGX_FILE" ]] || {
  printf 'no .lgx file found in %s\n' "$LGX_STORE" >&2
  exit 1
}
LGX_OUTPUT="$OUTPUT_DIR/$(basename "$LGX_FILE")"
docker cp "$CONTAINER:$LGX_FILE" "$LGX_OUTPUT" >/dev/null

printf 'Running official Basecamp integration output %s\n' "$INTEGRATION_ATTR"
INTEGRATION_STORE=$(build_attr "$INTEGRATION_ATTR")
if [[ -e "$OUTPUT_DIR/integration" ]]; then
  chmod -R u+w "$OUTPUT_DIR/integration"
fi
rm -rf "$OUTPUT_DIR/integration"
mkdir -p "$OUTPUT_DIR/integration"
docker exec "$CONTAINER" tar -C "$INTEGRATION_STORE" -cf - . |
  tar -C "$OUTPUT_DIR/integration" --no-same-owner --no-same-permissions -xf -

SCREENSHOT=$(find "$OUTPUT_DIR/integration" \
  -type f -name 'proofgate-desktop.png' -size +0c -print -quit)
[[ -n "$SCREENSHOT" ]] || {
  echo "official integration output did not contain a non-empty proofgate-desktop.png" >&2
  exit 1
}

{
  printf 'requested_image=%s\n' "$IMAGE"
  printf 'container_image=%s\n' "$CONTAINER_IMAGE"
  printf 'container_image_id=%s\n' "$CONTAINER_IMAGE_ID"
  printf 'container=%s\n' "$CONTAINER"
  printf 'nix_max_jobs=%s\n' "$NIX_MAX_JOBS"
  printf 'nix_cores=%s\n' "$NIX_CORES"
  printf 'lgx_store=%s\n' "$LGX_STORE"
  printf 'integration_store=%s\n' "$INTEGRATION_STORE"
} >"$OUTPUT_DIR/BUILD.txt"
sha256sum "$LGX_OUTPUT" "$SCREENSHOT" >"$OUTPUT_DIR/SHA256SUMS"

printf 'Basecamp local validation passed\n'
printf 'LGX: %s\n' "$LGX_OUTPUT"
printf 'Screenshot: %s\n' "$SCREENSHOT"

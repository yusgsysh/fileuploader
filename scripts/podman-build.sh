#!/usr/bin/env bash
set -euo pipefail

# Build the FileUploader image locally with Podman/Buildah.
# Usage:
#   ./scripts/podman-build.sh
#   IMAGE=fileuploader:dev ./scripts/podman-build.sh --no-cache
#   ./scripts/podman-build.sh --platform linux/amd64
#
# The script only builds; it does not log in to or push to a registry.

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
IMAGE="${IMAGE:-fileuploader:local}"
CONTAINERFILE="${CONTAINERFILE:-$ROOT_DIR/Containerfile}"

exec podman build \
  --layers \
  --file "$CONTAINERFILE" \
  --tag "$IMAGE" \
  "$@" \
  "$ROOT_DIR"

#!/usr/bin/env bash
set -euo pipefail

# Starts a disposable MinIO instance and runs the S3 multipart integration test.
# Requires Docker. The test creates the bucket and removes its object afterwards.

PORT="${MINIO_TEST_PORT:-19000}"
NAME="fileuploader-minio-test-$$"

cleanup() {
  docker rm -f "$NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT

docker run -d --rm --name "$NAME" \
  -p "${PORT}:9000" \
  -e MINIO_ROOT_USER=minioadmin \
  -e MINIO_ROOT_PASSWORD=minioadmin \
  minio/minio:latest server /data >/dev/null

for _ in $(seq 1 60); do
  if curl -fsS "http://127.0.0.1:${PORT}/minio/health/live" >/dev/null 2>&1; then
    break
  fi
  sleep 0.5
done

if ! curl -fsS "http://127.0.0.1:${PORT}/minio/health/live" >/dev/null 2>&1; then
  echo "MinIO did not become ready" >&2
  exit 1
fi

TEST_S3_ENDPOINT="http://127.0.0.1:${PORT}" \
TEST_S3_REGION="us-east-1" \
TEST_S3_ACCESS_KEY="minioadmin" \
TEST_S3_SECRET_KEY="minioadmin" \
TEST_S3_BUCKET="downloads" \
TEST_S3_PATH_STYLE="true" \
cargo test --test s3_integration -- --nocapture

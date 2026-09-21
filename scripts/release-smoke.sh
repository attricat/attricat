#!/usr/bin/env bash
# Exercise the release binaries against the same PostgreSQL and S3 settings
# used by deployment. The API applies its embedded SQLx migrations at startup.
set -euo pipefail

api_binary="${API_BINARY:-target/release/api}"
worker_binary="${FILE_WORKER_BINARY:-target/release/file-worker}"
smoke_dir="${CI_SMOKE_ARTIFACT_DIR:-artifacts/release-smoke}"
bind_addr="${BIND_ADDR:-127.0.0.1:3000}"
api_url="http://${bind_addr}"
mkdir -p "$smoke_dir"

for binary in "$api_binary" "$worker_binary"; do
  test -x "$binary" || {
    echo "Missing release binary: $binary" >&2
    exit 1
  }
done

api_pid=""
worker_pid=""
cleanup() {
  local status=$?
  for pid in "$worker_pid" "$api_pid"; do
    if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
    fi
  done
  exit "$status"
}
trap cleanup EXIT INT TERM

"$api_binary" >"$smoke_dir/api.log" 2>&1 &
api_pid=$!
for _ in {1..120}; do
  if curl --fail --silent --show-error "$api_url/health" >"$smoke_dir/health.json"; then
    break
  fi
  if ! kill -0 "$api_pid" 2>/dev/null; then
    wait "$api_pid"
  fi
  sleep 1
done

test -s "$smoke_dir/health.json" || {
  echo "Release API did not become healthy" >&2
  exit 1
}

# The worker has no HTTP listener; keeping it alive after storage/database
# initialization proves the release worker artifact can join the deployment.
"$worker_binary" >"$smoke_dir/file-worker.log" 2>&1 &
worker_pid=$!
sleep 3
kill -0 "$worker_pid" 2>/dev/null || {
  wait "$worker_pid"
  echo "Release file worker exited during startup" >&2
  exit 1
}

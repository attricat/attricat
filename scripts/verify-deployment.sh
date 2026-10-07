#!/usr/bin/env bash
# Disposable release-path exercise used by CI and available to operators.
# Database/object backup and restore are deployment-platform responsibilities;
# see the Operate section of the docs site for restore rehearsal.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$root"
compose=(docker compose -f deploy/compose.ci.yml)
image="${ATTRICAT_IMAGE:-attricat:ci}"
evidence="${DEPLOYMENT_EVIDENCE_DIR:-deployment-evidence}"
session_cookies="$(mktemp)"
mkdir -p "$evidence"
cleanup() {
  rm -f "$session_cookies"
  "${compose[@]}" down --volumes --remove-orphans >/dev/null 2>&1 || true
}
trap cleanup EXIT

if [[ "${SKIP_IMAGE_BUILD:-false}" != "true" ]]; then
  docker build --build-arg VCS_REF="$(git rev-parse HEAD)" --build-arg VCS_BRANCH="${GITHUB_HEAD_REF:-${GITHUB_REF_NAME:-$(git rev-parse --abbrev-ref HEAD)}}" --build-arg VERSION="$(git describe --always --dirty)" -t "$image" .
fi
export ATTRICAT_IMAGE="$image"

run_app() {
  docker run --rm --read-only --tmpfs /tmp:size=256m,mode=1777 \
    --security-opt no-new-privileges "$@"
}
wait_url() {
  local url=$1
  for _ in $(seq 1 90); do curl --fail --silent "$url" >/dev/null 2>&1 && return 0; sleep 2; done
  "${compose[@]}" logs >&2
  return 1
}
assert_status() {
  local expected=$1 url=$2
  local actual
  actual="$(curl --max-time 40 --silent --output /dev/null --write-out '%{http_code}' "$url")"
  [[ "$actual" == "$expected" ]] || {
    echo "expected HTTP $expected from $url, received $actual" >&2
    return 1
  }
}
assert_container_hardening() {
  local service=$1 container
  container="$("${compose[@]}" ps -q "$service")"
  [[ -n "$container" ]]
  [[ "$(docker inspect --format '{{.Config.User}}' "$container")" == "10001:10001" ]]
  [[ "$(docker inspect --format '{{.HostConfig.ReadonlyRootfs}}' "$container")" == "true" ]]
  docker inspect --format '{{json .HostConfig.SecurityOpt}}' "$container" | grep -q 'no-new-privileges'
}

[[ "$(docker image inspect --format '{{.Config.User}}' "$image")" == "10001:10001" ]]
[[ "$(run_app --entrypoint /usr/bin/id "$image" -u)" == "10001" ]]
# The application artifact must not contain database or object-storage utility
# binaries. Operators use their platform's separately maintained tooling.
run_app --entrypoint /bin/sh "$image" -c \
  '! command -v postgres && ! command -v pg_dump && ! command -v pg_restore && ! command -v aws'

"${compose[@]}" up -d postgres rustfs rustfs-init mailpit
"${compose[@]}" run --rm migrate
"${compose[@]}" up -d api file-worker
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready
assert_container_hardening api
assert_container_hardening file-worker
curl --fail --silent http://127.0.0.1:3000/ | grep -qi '<div id="root"'
curl --fail --silent http://127.0.0.1:3000/api/health/ready | grep -q ready
curl --fail --silent --cookie-jar "$session_cookies" \
  --header 'content-type: application/json' \
  --data '{"login_identifier":"default.local","email":"owner@example.test","password":"test-release-password"}' \
  http://127.0.0.1:3000/api/auth/login >/dev/null
for path in auth/session blueprints contexts workspace/navigation/sidebar extensions/runtime; do
  curl --fail --silent --cookie "$session_cookies" "http://127.0.0.1:3000/api/$path" >/dev/null
done
assert_status 401 http://127.0.0.1:3001/metrics
curl --fail --silent -H 'Authorization: Bearer release-test-token' http://127.0.0.1:3001/metrics \
  | grep -q catalog_file_worker_queue_depth

# Dependency outages withdraw readiness for both roles without changing liveness.
"${compose[@]}" stop rustfs
assert_status 200 http://127.0.0.1:3000/health/live
assert_status 503 http://127.0.0.1:3000/health/ready
assert_status 200 http://127.0.0.1:3001/health/live
assert_status 503 http://127.0.0.1:3001/health/ready
"${compose[@]}" start rustfs
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready
"${compose[@]}" stop postgres
assert_status 200 http://127.0.0.1:3000/health/live
assert_status 503 http://127.0.0.1:3000/health/ready
assert_status 200 http://127.0.0.1:3001/health/live
assert_status 503 http://127.0.0.1:3001/health/ready
"${compose[@]}" start postgres
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready

# Exercise rollback orchestration with a distinct retained-release fixture image.
# It intentionally shares application binaries with the candidate; this verifies
# identity replacement and rollback gating, not cross-version compatibility.
rollback_image=attricat:rollback-candidate
rollback_version="rollback-fixture-$(git rev-parse --short=12 HEAD)"
printf 'ARG BASE_IMAGE\nFROM ${BASE_IMAGE}\nLABEL org.opencontainers.image.version="%s"\n' "$rollback_version" \
  | docker build --build-arg BASE_IMAGE="$image" -t "$rollback_image" -
image_id="$(docker image inspect --format '{{.Id}}' "$image")"
rollback_image_id="$(docker image inspect --format '{{.Id}}' "$rollback_image")"
[[ "$image_id" != "$rollback_image_id" ]]
[[ "$(docker image inspect --format '{{index .Config.Labels "org.opencontainers.image.version"}}' "$rollback_image")" == "$rollback_version" ]]
ATTRICAT_IMAGE="$rollback_image" docker compose -f deploy/compose.ci.yml up -d --force-recreate api file-worker
wait_url http://127.0.0.1:3000/health/ready
wait_url http://127.0.0.1:3001/health/ready
api_container="$(ATTRICAT_IMAGE="$rollback_image" "${compose[@]}" ps -q api)"
[[ "$(docker inspect --format '{{.Image}}' "$api_container")" == "$rollback_image_id" ]]
{
  echo "image=$image"
  echo "image_id=$image_id"
  echo "rollback_fixture_version=$rollback_version"
  echo "rollback_image_id=$rollback_image_id"
  echo 'smoke=passed'
  echo 'api_alias_authorization=passed'
  echo 'dependency_outages=passed'
  echo 'runtime_utilities_absent=passed'
  echo 'rollback_seam=passed'
} | tee "$evidence/result.txt"

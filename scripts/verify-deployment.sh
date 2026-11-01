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

# The release path must also write: a record and a file through the running
# API and object store, and a real email through the mail relay.
csrf="$(awk '$6 == "attricat_csrf" { print $7 }' "$session_cookies")"
token="$(curl --fail --silent --cookie "$session_cookies" --header "x-attricat-csrf: $csrf" \
  --header 'content-type: application/json' \
  --data '{"label":"release-test","permissions":["blueprints.read","blueprints.write","blueprints.publish","records.read","records.write","records.publish"]}' \
  http://127.0.0.1:3000/api/personal-access-tokens | jq -r .secret)"
[[ -n "$token" && "$token" != null ]]
acli() {
  "${compose[@]}" exec -T -e ATTRICAT_API_URL=http://127.0.0.1:3000/api -e "ATTRICAT_TOKEN=$token" api acli "$@"
}
write_in_api() {
  "${compose[@]}" exec -T api sh -c "cat > /tmp/$1"
}
write_in_api smoke-blueprint.yaml <<'YAML'
format_version: 1
code: release_smoke
name: Release smoke
kind: record
attributes:
  - code: name
    value_type: string
  - code: attachment
    value_type: file
views:
  dropdown_option:
    type: dropdown_option
    fields: [name]
  detail:
    type: stack
    children:
      - {type: heading, text: Release smoke}
      - {type: field, field: name}
  table:
    type: table
    columns:
      - {field: name}
YAML
write_in_api smoke-values.yaml <<'YAML'
values:
  - kind: scalar
    attribute_code: name
    value: Release smoke
YAML
blueprint_id="$(acli blueprint create --file /tmp/smoke-blueprint.yaml | jq -r .blueprint.id)"
acli blueprint publish "$blueprint_id" 1 >/dev/null
record_id="$(acli record create --blueprint release_smoke --values /tmp/smoke-values.yaml \
  --context-id 00000000-0000-4000-8000-000000000001 | jq -r .id)"
curl --fail --silent --header "Authorization: Bearer $token" \
  "http://127.0.0.1:3000/api/v1/records/$record_id" | grep -q 'Release smoke'
# A 1x1 PNG, compared byte for byte after going through the object store.
smoke_png='iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=='
printf '%s' "$smoke_png" | base64 -d | write_in_api smoke.png
file_id="$(acli file upload "$record_id" attachment --file /tmp/smoke.png | jq -r '(.files // .)[0].id // .id')"
# The API serves a file once the file worker has prepared it.
for _ in $(seq 1 60); do
  [[ "$(curl --fail --silent --header "Authorization: Bearer $token" \
    "http://127.0.0.1:3000/api/files/$file_id/download" | base64 -w0 || true)" == "$smoke_png" ]] \
    && file_served=true && break
  sleep 1
done
[[ "${file_served:-false}" == true ]]
curl --fail --silent --output /dev/null --header 'content-type: application/json' \
  --data '{"email":"owner@example.test"}' http://127.0.0.1:3000/api/auth/password-reset
for _ in $(seq 1 30); do
  "${compose[@]}" exec -T mailpit wget -qO- http://127.0.0.1:8025/api/v1/messages \
    | grep -q 'owner@example.test' && mail_delivered=true && break
  sleep 1
done
[[ "${mail_delivered:-false}" == true ]]

assert_status 401 http://127.0.0.1:3000/api/metrics
curl --fail --silent -H 'Authorization: Bearer release-test-api-metrics-token' http://127.0.0.1:3000/api/metrics \
  | grep -q attricat_http_requests_total
assert_status 401 http://127.0.0.1:3001/metrics
curl --fail --silent -H 'Authorization: Bearer release-test-token' http://127.0.0.1:3001/metrics \
  | grep -q attricat_file_worker_queue_depth

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
  echo 'writes=passed'
  echo 'api_alias_authorization=passed'
  echo 'dependency_outages=passed'
  echo 'runtime_utilities_absent=passed'
  echo 'rollback_seam=passed'
} | tee "$evidence/result.txt"

#!/usr/bin/env bash
# Destructive, noninteractive restoration of the supplied Compose deployment.
set -Eeuo pipefail

if [[ ${1:-} == --help ]]; then
  echo "Usage: $0 [deployment-directory]"
  echo "Wipes this deployment's database and object storage; restores owner@example.com / test."
  exit 0
fi
[[ $# -le 1 ]] || { echo "Expected at most one deployment directory" >&2; exit 2; }
cd "${1:-$(cd "$(dirname "$0")" && pwd)}"
for tool in docker jq; do command -v "$tool" >/dev/null || { echo "Missing $tool" >&2; exit 1; }; done
[[ -f compose.yml && -f .env && -d reset && -d traefik/dynamic ]] || { echo "Incomplete demo deployment in $PWD" >&2; exit 1; }

# mkdir is atomic and works on Linux and macOS. A hard kill leaves the lock in
# place intentionally: verify no reset is running before removing a stale lock.
if ! mkdir .reset-lock 2>/dev/null; then
  echo "Another reset is running, or .reset-lock needs operator recovery" >&2
  exit 1
fi
printf '%s\n' "$$" > .reset-lock/pid
started=$SECONDS
phase=preflight
closed=false
success=false
dc=(docker compose --project-directory "$PWD")
export ATTRICAT_OWNER_EMAIL=owner@example.com ATTRICAT_OWNER_PASSWORD=test

log() { printf '%s %s\n' "$(date -u +%FT%TZ)" "$*"; }
maintenance_on() {
  # Atomic rename inside the watched directory; never show Traefik partial YAML.
  # shellcheck disable=SC2016 # Traefik's backticks are literal, not shell syntax.
  printf '%s\n' 'http:
  routers:
    demo-maintenance:
      entryPoints: [websecure]
      rule: "PathPrefix(`/`)"
      priority: 2147482647
      service: demo-maintenance
      tls: {}
  services:
    demo-maintenance:
      loadBalancer:
        servers:
          - url: "http://maintenance:8080"' > traefik/dynamic/maintenance.tmp
  mv traefik/dynamic/maintenance.tmp traefik/dynamic/maintenance.yml
}
finish() {
  result=$?
  trap - EXIT INT TERM
  if [[ $success != true ]]; then
    log "RESET FAILED phase=$phase exit=$result"
    if [[ $closed == true ]]; then
      maintenance_on || true
      "${dc[@]}" restart traefik || true
      # Also stop the writers, including if proxy reconfiguration failed.
      "${dc[@]}" stop api file-worker || true
      log "Left in maintenance. Fix the failure and rerun reset.sh."
    fi
    if [[ -n ${DEMO_RESET_FAILURE_HOOK:-} ]]; then
      "$DEMO_RESET_FAILURE_HOOK" "$PWD" "$phase" || log "Failure notification hook failed"
    fi
  fi
  jq -n --arg phase "$phase" --arg image "${ATTRICAT_IMAGE:-}" \
    --arg finished_at "$(date -u +%FT%TZ)" --argjson success "$success" \
    --argjson duration_seconds "$((SECONDS - started))" \
    '{success:$success,phase:$phase,image:$image,finished_at:$finished_at,duration_seconds:$duration_seconds}' > .reset-status.json || true
  rm -f .reset-lock/pid
  rmdir .reset-lock || true
  exit "$result"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Preserve the running application image even when :latest has moved locally.
# Reset is not an upgrade, and never pulls anything from a registry.
api_id=$("${dc[@]}" ps -a -q api)
if [[ -n $api_id ]]; then
  ATTRICAT_IMAGE=$(docker inspect --format '{{.Image}}' "$api_id")
else
  configured_image=$("${dc[@]}" config --format json | jq -er '.services.api.image')
  ATTRICAT_IMAGE=$(docker image inspect --format '{{.Id}}' "$configured_image")
fi
export ATTRICAT_IMAGE
log "Reset starting directory=$PWD image=$ATTRICAT_IMAGE"
while IFS= read -r image; do docker image inspect "$image" >/dev/null; done < <("${dc[@]}" config --images)
# Resolve the exact Compose volume names before stopping anything. External
# volumes are intentionally not managed by this workflow.
config=$("${dc[@]}" config --format json)
volumes=()
for key in postgres-data rustfs-data; do
  volumes+=("$(jq -er --arg key "$key" '.volumes[$key] | select(.external != true) | .name' <<< "$config")")
done
unset config
"${dc[@]}" run --rm --no-deps loader validate

phase=maintenance
closed=true
maintenance_on
"${dc[@]}" up -d --pull never traefik maintenance
# Explicit reload also works on VM-backed mounts without filesystem events.
"${dc[@]}" restart traefik
"${dc[@]}" run --rm --no-deps --entrypoint node loader /fixture/maintenance.mjs closed

phase=wipe
log "Stopping writers and clearing PostgreSQL, object storage, and captured mail"
"${dc[@]}" stop api file-worker migrate rustfs-init mailpit postgres rustfs
"${dc[@]}" rm -f api file-worker migrate rustfs-init mailpit postgres rustfs
for volume in "${volumes[@]}"; do
  # A first-ever reset may have no volumes yet. Other errors must not be hidden.
  if docker volume ls --format '{{.Name}}' | grep -Fxq "$volume"; then docker volume rm "$volume"; fi
done

phase=bootstrap
"${dc[@]}" up -d --pull never --wait --wait-timeout 300 api file-worker mailpit
phase=load
"${dc[@]}" run --rm --no-deps loader load
phase=reopen
rm traefik/dynamic/maintenance.yml
"${dc[@]}" restart traefik
"${dc[@]}" run --rm --no-deps --entrypoint node loader /fixture/maintenance.mjs open
success=true
phase=complete
log "RESET COMPLETE duration_seconds=$((SECONDS - started))"

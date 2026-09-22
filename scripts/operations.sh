#!/usr/bin/env bash
# Platform-neutral release hooks. Backup and restore are intentionally owned by
# the deployment platform; see docs/operations.md for the required procedure.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: operations.sh <rollout|migrate|rollback|rotate-secrets|dlq-list|dlq-replay>

Deployment hooks are trusted operator-provided commands: DEPLOY_COMMAND,
ROLLBACK_COMMAND, and ROTATE_SECRETS_COMMAND. Set READINESS_URL for rollout,
rollback, and rotation. Set APP_IMAGE and DATABASE_URL when running migrate
outside the application image.
EOF
}
need() { [[ -n "${!1:-}" ]] || { echo "$1 must be set" >&2; exit 2; }; }
run_hook() { bash -c "${!1}"; }
wait_ready() {
  local url="${READINESS_URL:?READINESS_URL must be set}"
  for _ in $(seq 1 "${READINESS_ATTEMPTS:-60}"); do
    if curl --fail --silent --show-error "$url" >/dev/null; then return 0; fi
    sleep 2
  done
  echo "readiness probe did not succeed: $url" >&2
  return 1
}
run_migrations() {
  need DATABASE_URL
  if command -v attricat-migrate >/dev/null 2>&1; then
    attricat-migrate
  else
    need APP_IMAGE
    docker run --rm --env DATABASE_URL "$APP_IMAGE" migrate
  fi
}

case "${1:-}" in
  rollout)
    need DEPLOY_COMMAND; need READINESS_URL
    run_hook DEPLOY_COMMAND
    wait_ready
    [[ -z "${SMOKE_COMMAND:-}" ]] || run_hook SMOKE_COMMAND
    ;;
  migrate)
    run_migrations
    ;;
  rollback)
    need ROLLBACK_COMMAND; need READINESS_URL
    run_hook ROLLBACK_COMMAND
    wait_ready
    [[ -z "${SMOKE_COMMAND:-}" ]] || run_hook SMOKE_COMMAND
    ;;
  rotate-secrets)
    need ROTATE_SECRETS_COMMAND; need READINESS_URL
    run_hook ROTATE_SECRETS_COMMAND
    wait_ready
    ;;
  dlq-list)
    exec acli event dead-letters
    ;;
  dlq-replay)
    [[ $# -eq 3 ]] || { echo 'usage: operations.sh dlq-replay <consumer-id> <event-id>' >&2; exit 2; }
    exec acli event replay "$2" "$3"
    ;;
  *) usage; exit 2 ;;
esac

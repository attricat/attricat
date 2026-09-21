#!/usr/bin/env bash
# Production operations entrypoint. Commands are deliberately explicit: this
# script never reads or prints secret values and refuses destructive restores
# without an operator confirmation.
set -euo pipefail

usage() {
  cat <<'EOF'
Usage: scripts/operations.sh <rollout|migrate|rollback|backup|restore|rotate-secrets|dlq-list|dlq-replay>

Required command hooks:
  DEPLOY_COMMAND, ROLLBACK_COMMAND, and ROTATE_SECRETS_COMMAND are deployment-
  platform commands supplied by CI or an operator. They must not echo secrets.
  DATABASE_URL is required for migrate, backup, and restore.
  S3_BACKUP_URI (for example s3://private-backups/catalog/2026-10-17) is
  required for backup and restore; AWS credentials are supplied externally.
EOF
}
need() { [[ -n "${!1:-}" ]] || { echo "$1 must be set" >&2; exit 2; }; }
wait_ready() {
  local url="${READINESS_URL:?READINESS_URL must be set}"
  for _ in $(seq 1 "${READINESS_ATTEMPTS:-60}"); do
    if curl --fail --silent --show-error "$url" >/dev/null; then return 0; fi
    sleep 2
  done
  echo "readiness probe did not succeed: $url" >&2
  return 1
}

case "${1:-}" in
  rollout)
    need DEPLOY_COMMAND; need READINESS_URL
    # The deployment platform performs a rolling replacement; only release
    # traffic after the new revision proves DB and object-storage readiness.
    bash -c "$DEPLOY_COMMAND"
    wait_ready
    ;;
  migrate)
    need DATABASE_URL
    sqlx migrate run --source apps/api/migrations --database-url "$DATABASE_URL"
    ;;
  rollback)
    need ROLLBACK_COMMAND; need READINESS_URL
    bash -c "$ROLLBACK_COMMAND"
    wait_ready
    ;;
  backup)
    need DATABASE_URL; need S3_BACKUP_URI
    stamp="$(date -u +%Y%m%dT%H%M%SZ)"
    workdir="${BACKUP_WORKDIR:-$(mktemp -d)}"
    trap 'rm -rf "$workdir"' EXIT
    pg_dump --format=custom --file "$workdir/catalog.dump" "$DATABASE_URL"
    # The bucket snapshot is intentionally paired with its matching DB dump.
    aws s3 sync "${S3_BUCKET_URI:?S3_BUCKET_URI must be set}" "$workdir/objects"
    aws s3 cp "$workdir/catalog.dump" "$S3_BACKUP_URI/$stamp/catalog.dump"
    aws s3 sync "$workdir/objects" "$S3_BACKUP_URI/$stamp/objects"
    printf 'backup=%s/%s\n' "$S3_BACKUP_URI" "$stamp"
    ;;
  restore)
    need DATABASE_URL; need S3_BACKUP_URI; need RESTORE_CONFIRM
    [[ "$RESTORE_CONFIRM" == "RESTORE" ]] || { echo 'set RESTORE_CONFIRM=RESTORE' >&2; exit 2; }
    workdir="${RESTORE_WORKDIR:-$(mktemp -d)}"
    trap 'rm -rf "$workdir"' EXIT
    aws s3 cp "$S3_BACKUP_URI/catalog.dump" "$workdir/catalog.dump"
    aws s3 sync "$S3_BACKUP_URI/objects" "$workdir/objects"
    pg_restore --clean --if-exists --no-owner --dbname "$DATABASE_URL" "$workdir/catalog.dump"
    aws s3 sync --delete "$workdir/objects" "${S3_BUCKET_URI:?S3_BUCKET_URI must be set}"
    ;;
  rotate-secrets)
    need ROTATE_SECRETS_COMMAND; need READINESS_URL
    # The platform rotates DB/S3/SMTP credentials then performs a controlled
    # rollout; this process never accepts secret material as an argument.
    bash -c "$ROTATE_SECRETS_COMMAND"
    wait_ready
    ;;
  dlq-list)
    cargo run -p cli -- event dead-letters
    ;;
  dlq-replay)
    [[ $# -eq 3 ]] || { echo 'usage: ... dlq-replay <consumer-id> <event-id>' >&2; exit 2; }
    cargo run -p cli -- event replay "$2" "$3"
    ;;
  *) usage; exit 2 ;;
esac

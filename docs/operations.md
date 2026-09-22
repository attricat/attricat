# Production operations

This runbook applies to the API, supervised task worker, file worker, PostgreSQL,
and the private S3-compatible bucket as one deployment unit. The API liveness
probe is `GET /health/live` (and the compatibility path `GET /health`): it only
means that the process can answer HTTP. Route traffic only when `GET
/health/ready` returns `200`; readiness actively checks PostgreSQL and the
configured object-store bucket and returns a sanitized `503` otherwise.

## Release and migration

Build an immutable image tagged with the Git commit SHA and retain its digest.
Run migrations exactly once before routing a new revision, then perform a
rolling rollout that keeps a prior healthy revision available. The supplied
entrypoint does not choose a deployment platform; CI supplies the platform
commands and probe URL:

```sh
export DATABASE_URL='postgres://...'
scripts/operations.sh migrate
export DEPLOY_COMMAND='./platform deploy --image registry/catalog:$GIT_SHA'
export READINESS_URL='https://catalog.example/health/ready'
scripts/operations.sh rollout
```

If readiness, smoke tests, or worker metrics regress, use the preceding image
digest, not a rebuilt tag:

```sh
export ROLLBACK_COMMAND='./platform rollback --image registry/catalog@sha256:...'
scripts/operations.sh rollback
```

Migrations are forward-only. A rollback reverts application code only; restore
the tested database/object backup rather than attempting to reverse schema
changes.

## Secrets and mail

Inject `DATABASE_URL`, `S3_ACCESS_KEY_ID`, `S3_SECRET_ACCESS_KEY`,
`SMTP_USERNAME`, and `SMTP_PASSWORD` from the deployment secret manager. Never
place them in images, arguments, logs, or this repository. `SMTP_TLS_MODE`
defaults to `starttls`; production accepts only `starttls` (port 587 normally)
or `implicit` (SMTPS, port 465 normally). `disabled` is solely for a trusted
local Mailpit relay and is set in `.env.example`.

Rotate a secret in the manager, validate the replacement credentials from a
new revision, roll workers and API, wait for readiness, then revoke the old
credential. CI can run the checked-in guard without passing secret values to
it:

```sh
export ROTATE_SECRETS_COMMAND='./platform rotate catalog-secrets'
export READINESS_URL='https://catalog.example/health/ready'
scripts/operations.sh rotate-secrets
```

## Metrics and DLQ

Scrape authenticated `GET /metrics` with a dedicated least-privilege token
from the private monitoring network. Alert on `catalog_task_worker_tasks_in_flight`, task and
operation duration histograms, task outcomes labelled `dead_letter` or
`lease_lost`, `catalog_extension_operations_total{outcome="failed"}`, and
`catalog_object_store_ready == 0`. Investigate logs and the owning extension
or workflow before replaying a dead letter; replay is at-least-once and must
be safe for the original idempotency key.

```sh
cargo run -p cli -- event dead-letters
cargo run -p cli -- event replay <consumer-id> <event-id>
# equivalent automation:
scripts/operations.sh dlq-list
scripts/operations.sh dlq-replay <consumer-id> <event-id>
```

## Backup and disposable restore exercise

Back up PostgreSQL and the object bucket as a pair. Supply private bucket URIs
and external AWS credentials; the helper writes a timestamped database dump
and matching object snapshot. It never writes credentials to disk.

```sh
export DATABASE_URL='postgres://...'
export S3_BUCKET_URI='s3://catalog-live-bucket'
export S3_BACKUP_URI='s3://catalog-backups/attricat'
scripts/operations.sh backup
```

At least once per release, restore one recorded backup into a disposable
PostgreSQL database and disposable bucket, run migrations, start a disposable
API/file-worker revision, and require `/health/ready` plus a known file and
operation-artifact download to succeed. The destructive restore path requires
an explicit confirmation:

```sh
export DATABASE_URL='postgres://...disposable...'
export S3_BUCKET_URI='s3://catalog-restore-check'
export S3_BACKUP_URI='s3://catalog-backups/attricat/20261017T120000Z'
export RESTORE_CONFIRM=RESTORE
scripts/operations.sh restore
scripts/operations.sh migrate
```

Extension package and operation output objects use immutable `v1` key prefixes.
New storage representations must introduce a new prefix and retain old readers
through the backup retention period; never reinterpret existing artifact bytes.

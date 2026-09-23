# Production operations

Attricat is distributed as one immutable image. Pin it by digest, just as a
PostgreSQL deployment pins its database image. The same image has three runtime
roles:

```sh
docker run --rm --env DATABASE_URL ghcr.io/attricat/attricat@sha256:… migrate
docker run … ghcr.io/attricat/attricat@sha256:… api
docker run … ghcr.io/attricat/attricat@sha256:… file-worker
```

The `api` role serves the compiled web application at `/`, the browser API below
`/api`, and compatibility API routes at their original paths. PostgreSQL,
private S3-compatible storage, SMTP, and monitoring are separately operated
services. `deploy/compose.yml` demonstrates API and file-worker roles using the
same image. Copy `deploy/.env.production.example` to the secret-managed
`deploy/.env.production`, replace every placeholder, and pass it both to Compose
interpolation and the services:

```sh
docker compose --env-file deploy/.env.production -f deploy/compose.yml up -d
```

The final image runs as uid/gid 10001, has a read-only-compatible filesystem,
and contains no build toolchain.

## Probes and rollout

API `GET /health/live` (and compatibility `GET /health`) only proves the process
can answer HTTP. Route traffic only when `GET /health/ready` is `200`; it probes
PostgreSQL and the configured bucket and returns only sanitized `503 not_ready`
on failure. The file worker provides the same live/ready split on its private
operations listener (port 3001 in the image).

Run the singleton `migrate` role before rolling API or worker replicas. Production
sets `CATALOG_AUTO_MIGRATE=false`; local development retains automatic migration.
Retain the prior image digest until smoke and restore verification complete:

```sh
export APP_IMAGE=ghcr.io/attricat/attricat@sha256:…
export DATABASE_URL='postgres://…'
scripts/operations.sh migrate
export DEPLOY_COMMAND='./platform deploy --image "$APP_IMAGE"'
export READINESS_URL='https://catalog.example/health/ready'
export SMOKE_COMMAND='./platform smoke catalog'
scripts/operations.sh rollout
```

Rollback means replacing application roles with the retained digest. Migrations
are forward-only, so rollback never runs reverse SQL. If application rollback is
not schema-compatible, restore the tested database/object pair instead:

```sh
export ROLLBACK_COMMAND='./platform deploy --image ghcr.io/attricat/attricat@sha256:previous'
scripts/operations.sh rollback
```

## Metrics and alerts

Keep API and worker metrics on a private monitoring network. API `/metrics`
requires a token/session with `data_health.read`. The file-worker `/metrics`
requires `Authorization: Bearer $FILE_WORKER_METRICS_TOKEN` whenever a token is
configured, and startup refuses a non-loopback listener without one.

Important series include:

- `catalog_database_ready` and `catalog_object_store_ready`;
- `catalog_file_worker_queue_depth`, `catalog_file_worker_oldest_age_seconds`,
  `catalog_file_worker_retries`, and completed/failed counters;
- `catalog_task_queue_depth`, `catalog_task_queue_oldest_age_seconds`,
  `catalog_task_queue_retries`, task outcomes, in-flight work, and duration;
- `catalog_extension_operation_runs`,
  `catalog_extension_operation_oldest_age_seconds`, retries, operation outcome,
  and duration.

Alert immediately when either readiness gauge is zero for two probe intervals,
when any `dead_letter`/`failed` depth is non-zero, or when worker failure counters
increase. Warn when queued oldest age exceeds 5 minutes for 10 minutes or when
queue depth grows for 15 minutes; tune these thresholds to expected import/export
volume. Alert on absence of worker metrics for two scrape intervals. Counters
provide throughput with Prometheus `rate(…[5m])`.

Inspect an event delivery before replay. Replay is at-least-once and the original
idempotency contract must still be safe:

```sh
acli event dead-letters
acli event replay <consumer-id> <event-id>
# Or use the image's CLI role:
docker run --rm … IMAGE acli event dead-letters
docker run --rm … IMAGE acli event replay <consumer-id> <event-id>
```

## SMTP and secret rotation

Use `SMTP_TLS_MODE=starttls` (normally 587) or `implicit` (normally 465).
Credentials with `disabled` are rejected; plaintext mode exists only for an
unauthenticated, trusted local Mailpit relay. Inject database, object-store,
SMTP, metrics, and bootstrap credentials from a secret manager—not an image,
Compose file, command argument, or log.

For rotation, stage replacement credentials, make them available to a new
revision, roll migration/API/worker roles, wait for readiness and smoke, then
revoke the old credential. The platform hook remains provider-neutral:

```sh
export ROTATE_SECRETS_COMMAND='./platform rotate-and-roll catalog-secrets'
export READINESS_URL='https://catalog.example/health/ready'
scripts/operations.sh rotate-secrets
```

## Coordinated backup and restore

Database rows and private objects form one consistency unit, but Attricat does
not bundle PostgreSQL or object-storage administration tools. The application
image intentionally contains no `postgres`, `pg_dump`, `pg_restore`, or `aws`
binary. Use separately maintained, version-matched tools supplied by the hosting
platform or short-lived administration containers.

A production backup procedure must:

1. Quiesce API writes and all workers at a known boundary.
2. Record the deployed Attricat image digest and timestamp.
3. Use a PostgreSQL 18-compatible `pg_dump --format=custom` from an external
   administration environment.
4. Snapshot the complete private object bucket while the application remains
   quiesced.
5. Produce a manifest pairing the database dump and object snapshot, including
   checksums and provider snapshot/version identifiers.
6. Resume the application only after both halves and the manifest are durable.

A restore rehearsal must use a disposable PostgreSQL database and bucket. Keep
the application quiesced, verify every checksum, restore both halves, run the
Attricat image's `migrate` role, then start API and file-worker roles against the
restored pair. Require both readiness probes and retrieve known database data, a
known uploaded file, and a known extension-operation artifact before recording
success. If any step fails, leave the target quiesced. Never restore only the
database or only the bucket.

Use provider-native snapshots when they can guarantee the same quiesced boundary.
Otherwise, run version-pinned utility containers outside the Attricat image. Keep
credentials in the platform secret manager and out of command arguments, logs,
manifests, and the application image. Rehearse this procedure at least once per
release and retain the resulting external evidence with the release digest.

Extension package and operation-output keys use immutable `v1` prefixes; new
representations must retain old readers through the backup retention period.

## Recorded release exercise

`scripts/verify-deployment.sh` builds the image, deploys disposable PostgreSQL,
RustFS and Mailpit dependencies, verifies production uid/read-only/no-new-privilege
hardening, confirms backup utility binaries are absent, exercises SPA/API/worker
probes, authenticated metrics, dependency outage semantics, and rollback gating
against a distinct fixture image identity. The fixture shares candidate binaries,
so it proves image replacement mechanics rather than cross-version compatibility.
CI uploads `deployment-evidence/` with image identities and smoke, outage, runtime
contents, and rollback-seam results. Backup/restore rehearsal evidence is owned
and retained by the deployment platform described above.

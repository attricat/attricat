---
title: Deploy Attricat
description: Run the Attricat image in production, with migrations, health checks, rollout, rollback, and secret rotation.
---

Attricat ships as one container image. The same image runs three roles:

| Role | Runs | Replicas |
| --- | --- | --- |
| `migrate` | Applies database migrations, then exits. | One, before each rollout. |
| `api` | The web app at `/`, the API at `/api` (and at its original paths), and all background workers except file processing. | One or more. |
| `file-worker` | Scans, processes, and cleans up uploaded files. | One or more. |

The image also contains the `acli` CLI as a fourth role.

You provide and operate the rest:

- **PostgreSQL 18**;
- a private **S3-compatible** bucket;
- an **SMTP** server with TLS;
- optionally an **OTLP** trace collector and **Prometheus**;
- optionally **Redis**, to share a cache between several API replicas.

## Pin the image

Deploy by digest, never by a moving tag:

```sh
docker run --rm --env-file attricat.env ghcr.io/attricat/attricat@sha256:… migrate
docker run -d --env-file attricat.env -p 3000:3000 ghcr.io/attricat/attricat@sha256:… api
docker run -d --env-file attricat.env ghcr.io/attricat/attricat@sha256:… file-worker
```

The image runs as uid and gid 10001, works with a read-only root file system, and contains no compilers or build tools. The API listens on port 3000; the file worker's private health and metrics listener is on port 3001.

## Docker Compose

The repository's `deploy/compose.yml` runs the API and file worker from one image. Copy `deploy/.env.production.example` to `deploy/.env.production`, replace every placeholder, keep the file in your secret manager, and start:

```sh
docker compose --env-file deploy/.env.production -f deploy/compose.yml up -d
```

Every setting is described in the [configuration reference](/reference/configuration/).

## First start

On first start the API creates the workspace named by `CATALOG_BOOTSTRAP_WORKSPACE_NAME` and an owner account for `CATALOG_BOOTSTRAP_OWNER_EMAIL`. Set `CATALOG_BOOTSTRAP_OWNER_PASSWORD` for that first start so the owner can sign in, then remove it. It never changes an existing password.

The owner signs in with the workspace identifier, normally `default.local`, then invites everyone else.

## Health checks

| Endpoint | Meaning | Use for |
| --- | --- | --- |
| `GET /health/live` (also `/health`) | The process answers HTTP. It does not check dependencies. | Liveness probe. |
| `GET /health/ready` | PostgreSQL and the bucket are reachable. Returns `503 not_ready` without details if not. | Readiness probe and load-balancer routing. |

The file worker serves the same two endpoints on its operations listener.

## Run several API replicas

API replicas can share one database and bucket.

**Database connections.** Besides its request and task pools, each API process may hold up to three more PostgreSQL connections. Rule schedules, workflow schedules and extension intake each run on one replica at a time, which keeps a lock on its own connection. The other replicas briefly connect every five seconds to check whether they should take over. Count these connections when you size PostgreSQL's `max_connections`. Connect directly to PostgreSQL or through a pooler in session mode; PgBouncer's transaction pooling is not supported.

**Shared cache (optional).** Each replica caches definitions in its own memory, which is always correct. To let replicas share cached values and extension network rate limits, run Redis and set:

```sh
CACHE_BACKEND=redis
REDIS_URL=rediss://:password@redis.example.com:6380/0
```

`rediss://` connects over TLS; `redis://` does not. Redis is never required: if it becomes unreachable, the API keeps working from memory and the database and reconnects on its own. Several Attricat deployments can share one Redis, because every key includes a random identifier of the deployment's database. A copy of a database keeps that identifier, so a deployment that runs on a copy of another one's database, such as staging cloned from production, must use a different `CACHE_KEY_PREFIX` or Redis database. After restoring a backup in place, flush Redis; see [Backup and restore](/operate/backup/).

## Roll out a new version

1. Keep the digest of the version you are running.
2. Run the new image's `migrate` role once. Production sets `CATALOG_AUTO_MIGRATE=false`, so API replicas never migrate on their own.
3. Replace the API and file-worker replicas with the new digest.
4. Wait for `/health/ready` and run your smoke tests.

With `./platform` standing in for your platform's deploy and smoke-test commands:

```sh
export APP_IMAGE=ghcr.io/attricat/attricat@sha256:…
export DATABASE_URL='postgres://…'
docker run --rm --env DATABASE_URL "$APP_IMAGE" migrate

./platform deploy --image "$APP_IMAGE"
curl --fail --silent --retry 60 --retry-delay 2 --retry-all-errors \
  https://catalog.example.com/health/ready
./platform smoke catalog
```

## Roll back

Migrations only go forward. Rolling back means redeploying the previous digest:

```sh
./platform deploy --image ghcr.io/attricat/attricat@sha256:previous
curl --fail --silent --retry 60 --retry-delay 2 --retry-all-errors \
  https://catalog.example.com/health/ready
```

If the previous version cannot run against the migrated database, restore the database and bucket from the backup taken before the rollout. See [Backup and restore](/operate/backup/).

## Rotate secrets

Inject database, bucket, SMTP, metrics, and bootstrap credentials from a secret manager, never from the image, a Compose file, a command-line argument, or a log.

To rotate: create the new credential, give it to a new revision of the deployment, roll out the migrate, API, and worker roles, wait for readiness and smoke tests, then revoke the old credential.

## Production checklist

- `SESSION_COOKIE_SECURE=true` and HTTPS in front of the API.
- `CATALOG_DEVTOOLS=false`.
- `CATALOG_AUTO_MIGRATE=false`, with `migrate` run before each rollout.
- `SMTP_TLS_MODE=starttls` or `implicit`.
- A real `CATALOG_BOOTSTRAP_OWNER_EMAIL`; `CATALOG_BOOTSTRAP_OWNER_PASSWORD` removed after first start.
- `FILE_WORKER_METRICS_TOKEN` set, and `/metrics` reachable only from your monitoring network.
- A private bucket, with credentials limited to `PutObject`, `GetObject`, `DeleteObject`, and `HeadBucket`.
- `CATALOG_E2E_FIXTURE_EMAIL` and `CATALOG_E2E_FIXTURE_PASSWORD` unset.
- A tested backup and restore procedure.
- With several API replicas: PostgreSQL `max_connections` sized for up to three extra connections per API process, no transaction-mode pooler, and optionally `CACHE_BACKEND=redis`.

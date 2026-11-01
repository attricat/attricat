---
title: Deploy Attricat
description: Run the Attricat image in production, with migrations, health checks, rollout, rollback, and secret rotation.
---

Attricat ships as one container image. The same image runs three roles:

| Role | Runs | Replicas |
| --- | --- | --- |
| `migrate` | Applies database migrations, then exits. | One, before each rollout. |
| `api` | The web app at `/`, the API at `/api`, and all background workers except file processing. | One or more. |
| `file-worker` | Scans, processes, and cleans up uploaded files. | One or more. |

The image also contains the `acli` CLI as a fourth role.

You provide and operate the rest:

- **PostgreSQL 18**;
- a private **S3-compatible** bucket;
- an **SMTP** server with TLS;
- optionally an **OTLP** trace collector and **Prometheus**;
- optionally **Redis**, to share a cache between several API replicas.

## Try it on your machine

To evaluate Attricat on one machine with Docker Compose, run:

```sh
curl -fsSL https://docs.attricat.com/install.sh | sh
```

The script creates an `attricat` directory in the current directory, writes `compose.yml`, a `.env` file and a `secrets` directory with a generated owner password, and starts the latest image with PostgreSQL, RustFS and Mailpit. When the stack is healthy, it prints the sign-in address, workspace, email and password. Email the server sends, such as password resets, shows up in Mailpit at <http://localhost:8025>.

To change a default, set the variable for `sh` on the first run, for example `curl -fsSL https://docs.attricat.com/install.sh | ATTRICAT_PORT=8080 sh`:

| Variable | Default |
| --- | --- |
| `ATTRICAT_DIR` | `attricat` |
| `ATTRICAT_IMAGE` | `ghcr.io/attricat/attricat:latest` |
| `ATTRICAT_PORT` | `3000` |
| `MAILPIT_UI_PORT` | `8025` |
| `ATTRICAT_OWNER_EMAIL` | `owner@example.com` |
| `ATTRICAT_OWNER_PASSWORD` | 24 random letters and digits |

The script saves these settings in `.env`, and the password in `secrets/bootstrap_owner_password`. Run it again to pull the newest image; it keeps `.env` and the secrets and rewrites `compose.yml`. From the `attricat` directory, `docker compose down` stops Attricat and `docker compose down -v` also deletes its data. The image is built for amd64 only, so Apple Silicon and other arm64 machines run it under emulation.

This setup binds to localhost and uses plain HTTP and fixed internal credentials, so use it only for evaluation. The rest of this page covers production.

## Pin the image

Deploy by digest, never by a moving tag. Settings come from an environment file and [secrets](/reference/configuration/#secrets) from a directory mounted at `/run/secrets`:

```sh
secrets="--volume /etc/attricat/secrets:/run/secrets:ro"
docker run --rm --env-file attricat.env $secrets ghcr.io/attricat/attricat@sha256:… migrate
docker run -d --env-file attricat.env $secrets -p 3000:3000 ghcr.io/attricat/attricat@sha256:… api
docker run -d --env-file attricat.env $secrets ghcr.io/attricat/attricat@sha256:… file-worker
```

Each release is also tagged with its version, such as `0.3.0`, and with its minor line, such as `0.3`, which moves to the latest patch release. From 1.0 a major tag, such as `1`, follows the latest release of that major version. Use these tags to find a release's digest, for example with `docker buildx imagetools inspect ghcr.io/attricat/attricat:0.3.0`, then deploy that digest. `latest` follows the main branch, not the latest release. The [releases page](https://github.com/attricat/attricat/releases) lists the changes in each version.

The image runs as uid and gid 10001, works with a read-only root file system, and contains no compilers or build tools. The API listens on port 3000; the file worker's private health and metrics listener is on port 3001.

## Docker Compose

The repository's `deploy/compose.yml` runs the API and file worker from one image and gives them every secret as a [Docker Compose secret](https://docs.docker.com/compose/how-tos/use-secrets/). Copy `deploy/.env.production.example` to `deploy/.env.production` and replace every placeholder. Then create one file per secret in `deploy/secrets`, or in the directory `ATTRICAT_SECRETS_HOST_DIR` names. Compose needs every file to exist; leave a secret you do not use empty. The container user, uid 10001, must be able to read them:

```sh
mkdir -m 700 deploy/secrets
cd deploy/secrets
printf '%s\n' 'postgres://attricat:…@postgres.example:5432/attricat' > database_url
printf '%s\n' '…' > s3_secret_access_key
printf '%s\n' '…' > smtp_password
printf '%s\n' '…' > file_worker_metrics_token
printf '%s\n' '…' > bootstrap_owner_password
touch llm_api_key redis_url extension_credential_key identity_provider_credential_key operator_api_token
sudo chown 10001:10001 * && sudo chmod 400 *
cd ../..
docker compose --env-file deploy/.env.production -f deploy/compose.yml up -d
```

Leave a file empty to turn its feature off. Single sign-on needs `identity_provider_credential_key`, extension connections need `extension_credential_key`, and the operator API needs `operator_api_token`. Every setting is described in the [configuration reference](/reference/configuration/).

## First start

On first start the API names the bootstrap workspace from `ATTRICAT_BOOTSTRAP_WORKSPACE_NAME` and creates an owner account for `ATTRICAT_BOOTSTRAP_OWNER_EMAIL`. Provide the `bootstrap_owner_password` secret for that first start so the owner can sign in, then empty it. It never changes an existing password.

The owner signs in with the workspace identifier, normally `default.local`, then invites everyone else. To add more workspaces, set the `operator_api_token` secret and use the [operator API](/reference/api/#operator-api).

## Health checks

| Endpoint | Meaning | Use for |
| --- | --- | --- |
| `GET /health/live` (also `/health`) | The process answers HTTP. It does not check dependencies. | Liveness probe. |
| `GET /health/ready` | PostgreSQL and the bucket are reachable. Returns `503 not_ready` without details if not. | Readiness probe and load-balancer routing. |

The file worker serves the same two endpoints on its operations listener.

## Run several API replicas

API replicas can share one database and bucket.

**Database connections.** Besides its request and task pools, each API process may hold up to three more PostgreSQL connections. Rule schedules, workflow schedules and extension intake each run on one replica at a time, which keeps a lock on its own connection. The other replicas briefly connect every five seconds to check whether they should take over. Count these connections when you size PostgreSQL's `max_connections`. Connect directly to PostgreSQL or through a pooler in session mode; PgBouncer's transaction pooling is not supported.

**Shared cache (optional).** Each replica caches definitions in its own memory, which is always correct. To let replicas share cached values and extension network rate limits, run Redis, set `CACHE_BACKEND=redis`, and put its URL in the `redis_url` secret:

```sh
printf '%s\n' 'rediss://:password@redis.example.com:6380/0' > deploy/secrets/redis_url
```

`rediss://` connects over TLS; `redis://` does not. Redis is never required: if it becomes unreachable, the API keeps working from memory and the database and reconnects on its own. Several Attricat deployments can share one Redis, because every key includes a random identifier of the deployment's database. A copy of a database keeps that identifier, so a deployment that runs on a copy of another one's database, such as staging cloned from production, must use a different `CACHE_KEY_PREFIX` or Redis database. After restoring a backup in place, flush Redis; see [Backup and restore](/operate/backup/).

## Roll out a new version

1. Keep the digest of the version you are running.
2. Run the new image's `migrate` role once. Production sets `ATTRICAT_AUTO_MIGRATE=false`, so API replicas never migrate on their own.
3. Replace the API and file-worker replicas with the new digest.
4. Wait for `/health/ready` and run your smoke tests.

With `./platform` standing in for your platform's deploy and smoke-test commands:

```sh
export APP_IMAGE=ghcr.io/attricat/attricat@sha256:…
docker run --rm --env-file attricat.env \
  --volume /etc/attricat/secrets:/run/secrets:ro "$APP_IMAGE" migrate

./platform deploy --image "$APP_IMAGE"
curl --fail --silent --retry 60 --retry-delay 2 --retry-all-errors \
  https://attricat.example.com/health/ready
./platform smoke catalog
```

### Upgrade from an image older than 2026-10-30

Definitions are YAML since then, and definitions stored as TOML are not converted. Reads keep working, but creating or changing a record on such a blueprint revision fails with `422 invalid_blueprint_definition`. For each affected blueprint, publish a new revision written in YAML, then move its records with `acli record migrate`.

## Roll back

Migrations only go forward. Rolling back means redeploying the previous digest:

```sh
./platform deploy --image ghcr.io/attricat/attricat@sha256:previous
curl --fail --silent --retry 60 --retry-delay 2 --retry-all-errors \
  https://attricat.example.com/health/ready
```

If the previous version cannot run against the migrated database, restore the database and bucket from the backup taken before the rollout. See [Backup and restore](/operate/backup/).

## Rotate secrets

Database, bucket, SMTP, agent, Redis, metrics, and bootstrap credentials are [secret files](/reference/configuration/#secrets) mounted at `/run/secrets`, never environment variables. Startup refuses a secret set as an environment variable. Write the files from your secret manager; never put a credential in the image, a Compose file, a command-line argument, or a log.

Secrets are read at startup. To rotate: create the new credential, replace its secret file, roll out the migrate, API, and worker roles, wait for readiness and smoke tests, then revoke the old credential.

## Production checklist

- `SESSION_COOKIE_SECURE=true` and HTTPS in front of the API.
- `ATTRICAT_DEVTOOLS=false`.
- `ATTRICAT_AUTO_MIGRATE=false`, with `migrate` run before each rollout.
- `SMTP_TLS_MODE=starttls` or `implicit`.
- A real `ATTRICAT_BOOTSTRAP_OWNER_EMAIL`; the `bootstrap_owner_password` secret emptied after first start.
- Secret files readable only by uid 10001, and no secrets in environment variables.
- The `file_worker_metrics_token` secret set, and `/metrics` reachable only from your monitoring network.
- A private bucket, with credentials limited to `PutObject`, `GetObject`, `DeleteObject`, `AbortMultipartUpload`, and `ListBucket`.
- `ATTRICAT_E2E_FIXTURE_EMAIL` unset.
- A proxy that sets `Strict-Transport-Security` and rate-limits `/api/auth/` per client address. Attricat limits guesses per account and workspace, which does not stop one client trying many of them.
- At least 2 GiB of memory for each API process. Password hashing is deliberately memory-hard, so a smaller limit can end the process under a burst of sign-ins.
- A tested backup and restore procedure.
- With several API replicas: PostgreSQL `max_connections` sized for up to three extra connections per API process, no transaction-mode pooler, and optionally `CACHE_BACKEND=redis`.

---
title: Configuration reference
description: Every environment variable read by the Attricat API, file worker, and web app.
---

Attricat is configured with environment variables. The API and the file worker read them at startup; nothing on this page can be changed from the web app or the API.

Both processes validate their configuration before they accept work. An invalid value, a missing required value, or an S3 bucket they cannot reach stops startup with an error rather than running with a guessed default.

`deploy/.env.production.example` in the repository is a starting point for a deployment. Copy it, replace every `replace-me` value, and keep the result in your secret manager.

## Minimum production configuration

A production API needs, at a minimum:

```sh
DATABASE_URL=postgres://attricat:…@postgres.example:5432/attricat
CATALOG_AUTO_MIGRATE=false
CATALOG_BOOTSTRAP_OWNER_EMAIL=owner@example.com
SESSION_COOKIE_SECURE=true
CATALOG_DEVTOOLS=false

S3_ENDPOINT=https://s3.example.com
S3_REGION=us-east-1
S3_BUCKET=attricat
S3_ACCESS_KEY_ID=…
S3_SECRET_ACCESS_KEY=…
S3_FORCE_PATH_STYLE=false
S3_UPLOAD_TIMEOUT_SECONDS=30
S3_DOWNLOAD_TIMEOUT_SECONDS=30

SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_TLS_MODE=starttls
MAIL_FROM="Attricat <no-reply@example.com>"
PASSWORD_RESET_URL=https://catalog.example.com/password-reset/confirm
WORKSPACE_INVITATION_URL=https://catalog.example.com/invitations/accept
WORKSPACE_ONBOARDING_URL=https://catalog.example.com/onboarding
```

The file worker needs the same `DATABASE_URL` and `S3_*` values plus `FILE_WORKER_METRICS_TOKEN`, because the container image binds its operations listener outside loopback.

## Server and database

| Variable | Default | Description |
| --- | --- | --- |
| `DATABASE_URL` | Required | PostgreSQL connection string. Attricat targets PostgreSQL 18. |
| `DATABASE_REQUEST_POOL_CONNECTIONS` | `10` | Connections in the pool that serves HTTP requests, shared by all workspaces. Integer from 1 to 100. |
| `DATABASE_TASK_POOL_CONNECTIONS` | `10` | Connections in the pool used by background workers, shared by all workspaces. Integer from 1 to 100. The API logs the total of the request, task, and maintenance pools at startup. Each API process may also hold up to three connections outside the pools for background coordinators, and these need session-mode connections: PgBouncer's transaction pooling is not supported. See [Run several API replicas](/operate/deployment/#run-several-api-replicas). |
| `BIND_ADDR` | `127.0.0.1:3000` | Address the API listens on. The container image sets `0.0.0.0:3000`. |
| `CATALOG_AUTO_MIGRATE` | `true` | Apply database migrations when the API starts. Set `false` in production and run the image's `migrate` role once before rolling out API replicas. |
| `WEB_DIST_DIR` | Unset | Directory containing the compiled web app. When set, the API serves the app at `/` and also exposes API routes below `/api`. The container image sets `/srv/attricat/web`. |
| `RUST_LOG` | `info` | Log filter, for example `api=debug`. |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | Unset | OTLP/gRPC endpoint for trace export from the API and file worker. Leave unset to disable tracing export. |
| `CATALOG_DEVTOOLS` | `true` in local development | Enables the web Inspector and the Explorer's SQL timing entries in the `Server-Timing` header. Set `false` in production. In other builds, including production, a browser can load the Inspector on demand by setting local storage `catalog.inspector-enabled` to `true` and reloading; SQL timings still require this setting on the API. SQL text and bind values are never exposed. |

## Bootstrap workspace and owner

On startup the API makes sure one workspace and its owner exist. These values are only read by the server; clients cannot supply them.

| Variable | Default | Description |
| --- | --- | --- |
| `CATALOG_WORKSPACE_ID` | `00000000-0000-4000-8000-000000000002` | UUID of the workspace created at startup. It does not select a workspace for HTTP requests; the signed-in session or token does that. |
| `CATALOG_BOOTSTRAP_WORKSPACE_NAME` | `Default workspace` | Display name used when the workspace is first created. |
| `CATALOG_BOOTSTRAP_OWNER_EMAIL` | `owner@example.test` | Email of the initial owner. It is trimmed and lowercased. Startup creates the user, membership, and owner grant if they do not exist. Always set a real address in a deployment. |
| `CATALOG_BOOTSTRAP_OWNER_ID` | Random UUID | Optional fixed UUID for the bootstrap owner. |
| `CATALOG_BOOTSTRAP_OWNER_PASSWORD` | Unset | Optional first password for a newly created owner. It is hashed before storage and never changes an existing password. Supply it for the first start only, then remove it. |
| `CATALOG_DEMO_MODE` | `false` | For public demo deployments only. Turns on `CATALOG_SAMPLE_ACCOUNTS`, gives the initial workspace the sign-in identifier `demo.attricat.com` and the default name `Demo`, and opens the sign-in page on that workspace with the editor account selected. Visitors can switch to the viewer, admin or owner account. Password reset, member and role-grant changes, ownership transfer, invitations and new users are turned off so visitors cannot lock the shared accounts out. Never enable it for a workspace with real data. |
| `CATALOG_SAMPLE_ACCOUNTS` | `false` | Creates `viewer@`, `editor@` and `admin@` accounts on the owner's email domain, each with that built-in role and the `CATALOG_BOOTSTRAP_OWNER_PASSWORD` password (which must then stay set). The sign-in page offers them in a picker. Anyone who can reach the server can sign in with them, so use it only for local development and demos. |

The bootstrap workspace's sign-in identifier is `default.local`.

## Sessions and security

| Variable | Default | Description |
| --- | --- | --- |
| `SESSION_COOKIE_SECURE` | `true` | Marks the session and CSRF cookies `Secure`. Set `false` only for local HTTP development. |

Session lifetime (eight hours), the login rate limit (five failures per workspace and email in fifteen minutes), and password-reset link lifetime (30 minutes) are fixed.

## HTTP limits

| Variable | Default | Description |
| --- | --- | --- |
| `HTTP_REQUEST_TIMEOUT_SECONDS` | `30` | Wall-clock limit for one request. A request that exceeds it returns `503` with code `request_timeout`. The write may still have happened, so check before retrying it. |
| `HTTP_MAX_CONCURRENT_REQUESTS` | `256` | Maximum requests in flight per API process. Extra requests get `503` immediately instead of queueing. Health checks and open event streams have their own limits. |
| `HTTP_MAX_EVENT_STREAMS` | `128` | Maximum open event streams per API process. |
| `HTTP_MAX_EVENT_STREAMS_PER_PRINCIPAL` | `4` | Maximum open event streams per user in a workspace, counted across all of that user's sessions and tokens. |
| `HTTP_EVENT_STREAM_LIFETIME_SECONDS` | `900` | Longest an event stream stays open. The API then closes it, and clients reconnect with `Last-Event-ID`. |
| `HTTP_DEFAULT_BODY_BYTES` | `2097152` (2 MiB) | Default request body limit. Upload routes use the file limits below instead. |

## Catalog behavior and limits

| Variable | Default | Description |
| --- | --- | --- |
| `PREVIEW_MAX_RELATIONSHIP_DEPTH` | `3` | Deepest relationship nesting an entity preview may request. |
| `PREVIEW_MAX_RELATIONSHIP_ITEMS` | `10` | Maximum related entities shown inline per relationship in a preview. |
| `ENTITY_MAX_PAGE_SIZE` | `100` | Largest page size for relationship target browsing. |
| `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` | `50` | Largest page size for incoming-relationship lists. Caps `page_size` in `incoming_relationship_list` view blocks. |
| `RELATIONSHIP_FACET_MAX_NODES` | `100` | Maximum nodes returned per page of an Explorer relationship facet. |
| `DATA_HEALTH_CACHE_TTL_SECONDS` | `300` | How long data-health responses are cached. `0` disables the cache. Any recorded catalog change also refreshes them on the next request. |
| `CACHE_BACKEND` | `memory` | Where cached definitions live: `memory` (each process) or `redis` (shared by every replica, which then also share extension network rate limits). Cached data is always correct with either backend. If Redis is unreachable the API keeps working from memory and reconnects on its own. |
| `REDIS_URL` | Unset | Redis connection URL. Required when `CACHE_BACKEND` is `redis`. Use `rediss://` (or `valkeys://`) for TLS. |
| `CACHE_MAX_ENTRIES` | `20000` | Maximum number of in-memory cache entries per process. |
| `CACHE_KEY_PREFIX` | `attricat` | First part of every Redis key; a random identifier of the database follows it. A copy of a database keeps that identifier, so a copy that runs alongside its source (such as staging cloned from production) needs a different prefix or Redis database. After restoring a backup in place, change it or flush Redis. |
| `ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS` | `90` | Days of attribute-value history kept. Every minute the API spends up to 10 seconds deleting older history, at most 1,000 rows per transaction. A failed cleanup is logged and tried again; it does not stop the API. |
| `BLUEPRINT_MIGRATION_PAGE_SIZE` | `100` | Entities read per page during a background blueprint migration. 1 to 1000. |
| `BLUEPRINT_MIGRATION_CONCURRENCY` | `4` | Entities migrated at the same time within one migration batch. 1 to 64. |

## Background work

The API runs a task worker for agent runs, rule runs, workflow runs, extension event deliveries, extension operations, and blueprint migrations. It also runs the event dispatcher that delivers internal domain events to their handlers.

| Variable | Default | Description |
| --- | --- | --- |
| `TASK_WORKER_ID` | Random per process | Stable name for this process's task worker, 1 to 128 bytes. |
| `TASK_WORKER_CONCURRENCY` | `8` | Tasks this process runs at the same time. |
| `TASK_WORKER_POLL_MILLIS` | `250` | Idle delay between polls for new tasks. |
| `TASK_WORKER_SHUTDOWN_GRACE_SECONDS` | `30` | On shutdown, how long running tasks may finish. Unfinished tasks are picked up again after their lease expires. |
| `EVENT_DISPATCHER_LEASE_SECONDS` | `30` | How long one delivery attempt holds its lease. A shorter lease increases the chance of duplicate delivery. |
| `EVENT_DISPATCHER_RETRY_INITIAL_SECONDS` | `1` | First retry delay after a failed delivery. Delays double on each failure. |
| `EVENT_DISPATCHER_RETRY_MAX_SECONDS` | `60` | Upper bound for the retry delay. |
| `EVENT_DISPATCHER_MAX_ATTEMPTS` | `5` | Attempts before a delivery becomes a dead letter. |
| `EVENT_DISPATCHER_POLL_MILLIS` | `250` | Delay between dispatcher polls. |

All values must be positive integers. Zero or a non-integer stops startup.

## Object storage

Attricat stores uploaded files, generated image variants, extension artifacts, and presentation assets in one private S3-compatible bucket. All `S3_*` variables are required by both the API and the file worker.

| Variable | Description |
| --- | --- |
| `S3_ENDPOINT` | Absolute HTTP(S) URL of the S3-compatible service. |
| `S3_REGION` | Signing region. |
| `S3_BUCKET` | Existing bucket name. Attricat does not create it in production. |
| `S3_ACCESS_KEY_ID` | Access key. |
| `S3_SECRET_ACCESS_KEY` | Secret key. Keep it in a secret manager. |
| `S3_FORCE_PATH_STYLE` | `true` or `false`. Use `true` for services that do not support virtual-hosted buckets, such as RustFS or MinIO. |
| `S3_UPLOAD_TIMEOUT_SECONDS` | Timeout for uploads and deletes. |
| `S3_DOWNLOAD_TIMEOUT_SECONDS` | Timeout for downloads and the startup bucket check. |

Grant the credentials only `PutObject`, `GetObject`, `DeleteObject`, and `HeadBucket` on that bucket, keep the bucket private, and use TLS. Clients never receive object keys or signed URLs; every download goes through the API.

## File uploads and processing

| Variable | Default | Used by | Description |
| --- | --- | --- | --- |
| `FILE_UPLOAD_MAX_BYTES` | `52428800` (50 MiB) | API | Largest single uploaded file. A file attribute's `max_bytes` can set a lower limit. |
| `FILE_UPLOAD_MAX_FILES` | `10` | API | Most files in one upload request. An attribute with `cardinality = "one"` accepts exactly one. |
| `FILE_WORKER_ID` | Random per process | File worker | Stable identifier written on jobs this worker claims. |
| `FILE_WORKER_POLL_MILLISECONDS` | `500` | File worker | Delay between job polls. |
| `FILE_WORKER_OPERATIONS_BIND_ADDR` | `127.0.0.1:3001` | File worker | Private listener for `/health/live`, `/health/ready`, and `/metrics`. |
| `FILE_WORKER_METRICS_TOKEN` | Unset | File worker | Bearer token required on `/metrics`. Startup refuses a non-loopback listener without it. |
| `FILE_WORKER_MAX_PIXELS` | `40000000` | File worker | Largest decoded image, in pixels, the worker will process. |
| `FILE_WORKER_MAX_ATTEMPTS` | `5` | File worker | Attempts before a processing job fails permanently. |
| `FILE_DELETE_GRACE_SECONDS` | `86400` (one day) | File worker | Time between an unreferenced file being marked deleted and its object being purged. |

A permanently failed processing job can be queued again. The attempt counter is reset only for a job that has failed permanently:

```sh
docker run --rm --env-file … ghcr.io/attricat/attricat@sha256:… file-worker --retry <job-uuid>
```

## Email

Attricat sends password-reset, invitation, and onboarding email over SMTP.

| Variable | Default | Description |
| --- | --- | --- |
| `SMTP_HOST` | `127.0.0.1` | SMTP server host. |
| `SMTP_PORT` | `1025` | SMTP server port. Normally `587` for `starttls` and `465` for `implicit`. |
| `SMTP_TLS_MODE` | `starttls` | `starttls` or `implicit`. `disabled` is accepted only for an unauthenticated local relay such as Mailpit; startup rejects credentials combined with `disabled`. |
| `SMTP_USERNAME` | Unset | SMTP username. Set both username and password, or neither. |
| `SMTP_PASSWORD` | Unset | SMTP password. |
| `MAIL_FROM` | `Catalog <no-reply@catalog.local>` | Sender address. |
| `PASSWORD_RESET_URL` | Local URL | Absolute URL of the web app's password-reset confirmation page, for example `https://catalog.example.com/password-reset/confirm`. |
| `WORKSPACE_INVITATION_URL` | Local URL | Absolute URL used in invitations to existing users, for example `https://catalog.example.com/invitations/accept`. |
| `WORKSPACE_ONBOARDING_URL` | Local URL | Absolute URL used in onboarding links for new users, for example `https://catalog.example.com/onboarding`. |

## Agents

Agents are off until `LLM_API_KEY` is set. The rest of the API starts normally without it. The provider must be compatible with the OpenAI Chat Completions API. See [Agents and approvals](/guides/agents/) for what data a provider receives.

| Variable | Default | Description |
| --- | --- | --- |
| `LLM_API_KEY` | Unset | Provider API key. Set it only in the API process environment. It is never stored or logged. |
| `LLM_BASE_URL` | `https://api.openai.com/v1` | Absolute base URL of the Chat Completions API. |
| `LLM_MODEL` | `gpt-4o-mini` | Model identifier. Recorded on every run. |
| `LLM_REASONING_EFFORT` | Unset | Optional `reasoning_effort` sent with every request. Some models reject function tools unless this is `none`. |
| `LLM_REQUEST_TIMEOUT_SECONDS` | `60` | Timeout for one provider request, 1 to 3600. |
| `LLM_RUN_TIMEOUT_SECONDS` | `300` | Timeout for a whole agent run, 1 to 3600. |

These agent limits are fixed: 32 KiB per user message, 16 attachments per message, 5 MiB per inline image, eight tool-call rounds per run, 64 KiB per serialized tool result, and 32 tool calls per provider response.

## Extensions

| Variable | Default | Description |
| --- | --- | --- |
| `EXTENSION_OFFICIAL_REGISTRY` | `attricat/attricat-extensions` | GitHub `owner/repository` used as every workspace's built-in extension registry. |
| `EXTENSIONS_MODE` | `enabled` | Emergency switch. Set exactly `disabled` to stop all extension execution, artifact delivery, commands, storage, host calls, and event delivery across the deployment. Installations and grants are left unchanged. Any other unrecognized value is treated as `disabled`. |
| `EXTENSION_DENYLIST` | Empty | Comma-separated list of extension IDs (`acme.inventory`) or exact releases (`acme.inventory@<release-uuid>`) to block. Checked on every runtime call. |

## Web app development server

| Variable | Default | Description |
| --- | --- | --- |
| `CATALOG_API_URL` | `http://127.0.0.1:3000` | API address the Vite development server proxies `/api` to. The CLI also reads it. |
| `WEB_PORT` | `5173` | Port of the Vite development server. |

## Local development services

The repository's local stack runs PostgreSQL, Mailpit (email capture), Jaeger (traces), and RustFS (S3-compatible storage) in containers. These variables only set their host ports and credentials.

| Variable | Default |
| --- | --- |
| `POSTGRES_DB` | `catalog` |
| `POSTGRES_USER` | `postgres` |
| `POSTGRES_PASSWORD` | `postgres` |
| `POSTGRES_PORT` | `5432` |
| `MAILPIT_SMTP_PORT` | `1025` |
| `MAILPIT_UI_PORT` | `8025` |
| `JAEGER_OTLP_GRPC_PORT` | `4317` |
| `JAEGER_UI_PORT` | `16686` |
| `RUSTFS_PORT` | `9000` |
| `RUSTFS_CONSOLE_PORT` | `9001` |

`CATALOG_E2E_FIXTURE_EMAIL` and `CATALOG_E2E_FIXTURE_PASSWORD` create a test user for end-to-end tests. Never set them in production.

## CLI

The `acli` command-line client reads its own variables. See the [CLI reference](/reference/cli/#configuration).

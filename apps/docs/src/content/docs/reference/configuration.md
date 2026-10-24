---
title: Configuration reference
description: Every environment variable and secret read by the Attricat API, file worker, and web app.
---

Attricat is configured with environment variables and secret files. The API, the file worker and the `migrate` role read them at startup; nothing on this page can be changed from the web app or the API.

Every process reads and validates the whole configuration before it accepts work. An invalid value, a missing required value, or an S3 bucket they cannot reach stops startup with an error rather than running with a guessed default.

`deploy/.env.production.example` in the repository is a starting point for a deployment. Copy it and replace every `replace-me` value. It holds no secrets: those go in [secret files](#secrets).

## Secrets

Attricat never reads a password, key or token from an environment variable. Each secret is a file named after it in `/run/secrets`, where Docker and Docker Compose mount [secrets](https://docs.docker.com/compose/how-tos/use-secrets/). A trailing line break is ignored, and a missing or empty file leaves an optional secret unset. The files must be readable by the container's user, uid `10001`.

| Secret | Default | Description |
| --- | --- | --- |
| `database_url` | Required | PostgreSQL connection string, including its password. Attricat targets PostgreSQL 18. |
| `s3_secret_access_key` | Required | Secret key for `S3_ACCESS_KEY_ID`. |
| `smtp_password` | Unset | SMTP password, used with `SMTP_USERNAME`. |
| `llm_api_key` | Unset | Agent provider API key. Agents are off without it. It is never stored or logged. |
| `redis_url` | Unset | Redis connection URL. Required when `CACHE_BACKEND` is `redis`. Use `rediss://` (or `valkeys://`) for TLS. |
| `bootstrap_owner_password` | Unset | Optional first password for a newly created owner. It is hashed before storage and never changes an existing password. Supply it for the first start only, then empty the file. |
| `file_worker_metrics_token` | Unset | Bearer token required on the file worker's `/metrics`. The file worker refuses a non-loopback operations listener without it. |
| `extension_credential_key` | Unset | 32 random bytes, base64-encoded, that encrypt extension [connection](/builders/extensions/#connections) credentials (client secrets, refresh and access tokens, device codes) with AES-256-GCM. Without it, connections cannot be created or used. Any other length stops startup. Keep it stable: a new key makes stored credentials unreadable, and every connection must be set up again. Generate one with `openssl rand -base64 32`. |

Startup stops if one of these is set as an environment variable, such as `LLM_API_KEY` or `SMTP_PASSWORD`, so a credential is never silently ignored. A `DATABASE_URL` variable is ignored. `ATTRICAT_SECRETS_DIR` reads the files from another directory instead of `/run/secrets`.

## Minimum production configuration

A production API needs, at a minimum, the `database_url` and `s3_secret_access_key` secrets, the `smtp_password` secret if the SMTP server needs one, and:

```sh
ATTRICAT_AUTO_MIGRATE=false
ATTRICAT_BOOTSTRAP_OWNER_EMAIL=owner@example.com
SESSION_COOKIE_SECURE=true
ATTRICAT_DEVTOOLS=false

S3_ENDPOINT=https://s3.example.com
S3_REGION=us-east-1
S3_BUCKET=attricat
S3_ACCESS_KEY_ID=…
S3_FORCE_PATH_STYLE=false
S3_UPLOAD_TIMEOUT_SECONDS=30
S3_DOWNLOAD_TIMEOUT_SECONDS=30

SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_TLS_MODE=starttls
MAIL_FROM="Attricat <no-reply@example.com>"
PASSWORD_RESET_URL=https://attricat.example.com/password-reset/confirm
WORKSPACE_INVITATION_URL=https://attricat.example.com/invitations/accept
WORKSPACE_ONBOARDING_URL=https://attricat.example.com/onboarding
```

The file worker reads the same configuration and also needs the `file_worker_metrics_token` secret when its operations listener binds outside loopback, as it does in `deploy/compose.yml`.

## Server and database

| Variable | Default | Description |
| --- | --- | --- |
| `DATABASE_REQUEST_POOL_CONNECTIONS` | `10` | Connections in the pool that serves HTTP requests, shared by all workspaces. Integer from 1 to 100. |
| `DATABASE_TASK_POOL_CONNECTIONS` | `10` | Connections in the pool used by background workers, shared by all workspaces. Integer from 1 to 100. The API logs the total of the request, task, and maintenance pools at startup. Each API process may also hold up to three connections outside the pools for background coordinators, and these need session-mode connections: PgBouncer's transaction pooling is not supported. See [Run several API replicas](/operate/deployment/#run-several-api-replicas). |
| `BIND_ADDR` | `127.0.0.1:3000` | Address the API listens on. The container image sets `0.0.0.0:3000`. |
| `ATTRICAT_AUTO_MIGRATE` | `true` | Apply database migrations when the API starts. Set `false` in production and run the image's `migrate` role once before rolling out API replicas. |
| `WEB_DIST_DIR` | Unset | Directory containing the compiled web app. When set, the API serves the app for every path outside `/api` and the health probes. The container image sets `/srv/attricat/web`. |
| `ATTRICAT_SECRETS_DIR` | `/run/secrets` | Directory holding the [secret files](#secrets). |
| `RUST_LOG` | `info` | Log filter, for example `api=debug`. |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | Unset | OTLP/gRPC endpoint for trace export from the API and file worker. Leave unset to disable tracing export. |
| `ATTRICAT_DEVTOOLS` | `true` in local development | Enables the web Inspector and the Explorer's SQL timing entries in the `Server-Timing` header. Set `false` in production. In other builds, including production, a browser can load the Inspector on demand by setting local storage `attricat.inspector-enabled` to `true` and reloading; SQL timings still require this setting on the API. SQL text and bind values are never exposed. |

## Bootstrap workspace and owner

On startup the API makes sure one workspace and its owner exist. These values are only read by the server; clients cannot supply them.

| Variable | Default | Description |
| --- | --- | --- |
| `ATTRICAT_WORKSPACE_ID` | `00000000-0000-4000-8000-000000000002` | UUID of the workspace created at startup. It does not select a workspace for HTTP requests; the signed-in session or token does that. |
| `ATTRICAT_BOOTSTRAP_WORKSPACE_NAME` | `Default workspace` | Display name used when the workspace is first created. |
| `ATTRICAT_BOOTSTRAP_OWNER_EMAIL` | `owner@example.test` | Email of the initial owner. It is trimmed and lowercased. Startup creates the user, membership, and owner grant if they do not exist. Always set a real address in a deployment. |
| `ATTRICAT_BOOTSTRAP_OWNER_ID` | Random UUID | Optional fixed UUID for the bootstrap owner. Its first password is the `bootstrap_owner_password` [secret](#secrets). |
| `ATTRICAT_DEMO_MODE` | `false` | For public demo deployments only. Turns on `ATTRICAT_SAMPLE_ACCOUNTS`, gives the initial workspace the sign-in identifier `demo.attricat.com` and the default name `Demo`, and opens the sign-in page on that workspace with the editor account selected. Visitors can switch to the viewer, admin or owner account. Password reset, member and role-grant changes, ownership transfer, invitations and new users are turned off so visitors cannot lock the shared accounts out. If agents are configured, they can read the catalog and explain changes but cannot make them. Never enable it for a workspace with real data. |
| `ATTRICAT_SAMPLE_ACCOUNTS` | `false` | Creates `viewer@`, `editor@` and `admin@` accounts on the owner's email domain, each with that built-in role and the `bootstrap_owner_password` secret as its password (which must then stay set). The sign-in page offers them in a picker. Anyone who can reach the server can sign in with them, so use it only for local development and demos. |

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

## Attricat behavior and limits

| Variable | Default | Description |
| --- | --- | --- |
| `PREVIEW_MAX_RELATIONSHIP_DEPTH` | `3` | Deepest relationship nesting a record preview may request. |
| `PREVIEW_MAX_RELATIONSHIP_ITEMS` | `10` | Maximum related records shown inline per relationship in a preview. |
| `RECORD_MAX_PAGE_SIZE` | `100` | Largest page size for relationship target browsing. |
| `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` | `50` | Largest page size for incoming-relationship lists. Caps `page_size` in `incoming_relationship_list` view blocks. |
| `RELATIONSHIP_FACET_MAX_NODES` | `100` | Maximum nodes returned per page of an Explorer relationship facet. |
| `DATA_HEALTH_CACHE_TTL_SECONDS` | `300` | How long data-health responses are cached. `0` disables the cache. Any recorded catalog change also refreshes them on the next request. |
| `CACHE_BACKEND` | `memory` | Where cached definitions live: `memory` (each process) or `redis` (shared by every replica, which then also share extension network rate limits). Cached data is always correct with either backend. If Redis is unreachable the API keeps working from memory and reconnects on its own. `redis` reads the URL from the `redis_url` [secret](#secrets). |
| `CACHE_MAX_ENTRIES` | `20000` | Maximum number of in-memory cache entries per process. |
| `CACHE_KEY_PREFIX` | `attricat` | First part of every Redis key; a random identifier of the database follows it. A copy of a database keeps that identifier, so a copy that runs alongside its source (such as staging cloned from production) needs a different prefix or Redis database. After restoring a backup in place, change it or flush Redis. |
| `BLUEPRINT_MIGRATION_PAGE_SIZE` | `100` | Records read per page during a background blueprint migration. 1 to 1000. |
| `BLUEPRINT_MIGRATION_CONCURRENCY` | `4` | Records migrated at the same time within one migration batch. 1 to 64. |

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

## Data retention

Every minute the API deletes old operational data in small batches: finished background work, delivered events, ended sessions and similar records. Each table group is cleaned in turn, so one large backlog cannot hold up the rest. Records that are still pending or running, and anything that live work still needs, are never deleted. A failed cleanup is logged and tried again on the next run; it does not stop the API.

| Variable | Default | What is deleted after this many days |
| --- | --- | --- |
| `RETENTION_SWEEP_INTERVAL_SECONDS` | `60` | How often cleanup starts, 1 to 86400 seconds. |
| `RETENTION_SWEEP_BUDGET_SECONDS` | `10` | Longest one cleanup runs, 1 to 3600 seconds and no longer than the interval. |
| `RETENTION_BATCH_ROWS` | `1000` | Most rows deleted per table in one transaction, 1 to 10000. Each statement must finish within 2 seconds, so raise the budget before raising this. |
| `ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS` | `90` | Earlier attribute values. Current values are never deleted. |
| `TASK_RETENTION_DAYS` | `7` | Finished background task records. The latest failed attempt stays until its run is deleted, so it can be retried. |
| `RULE_RUN_RECORD_RETENTION_DAYS` | `7` | Finished rule checks of a single record. |
| `RULE_RUN_RETENTION_DAYS` | `90` | Finished full rule runs and failed rule runs. Each rule version keeps its latest run and its latest completed dry run, so enabling it again does not need a new dry run. |
| `WORKFLOW_RUN_RETENTION_DAYS` | `90` | Finished workflow runs. |
| `EXTENSION_OPERATION_RUN_RETENTION_DAYS` | `90` | Finished extension operations whose outputs have already expired. |
| `AGENT_RUN_EVENT_RETENTION_DAYS` | `90` | The live progress stream of finished agent runs. Conversations, their messages and tool calls stay. |
| `FILE_PROCESSING_JOB_RETENTION_DAYS` | `30` | Finished file metadata jobs. File purge jobs stay. |
| `SOLUTION_PACK_CHECK_RUN_RETENTION_DAYS` | `90` | Manual solution pack check runs. Each application keeps its post-apply check and its latest check. |
| `DOMAIN_EVENT_RETENTION_DAYS` | `90` | Events that every handler and extension has received. Events with an undelivered or failed delivery stay, so failed deliveries can always be replayed. An extension that resumes the change feed from a cursor issued before events after it were deleted gets a "cursor has expired" error and must take a new snapshot. An event handler or extension that no longer runs holds back cleanup for its workspace until it is removed. |
| `BROWSER_SESSION_RETENTION_DAYS` | `7` | Expired and signed-out browser sessions. |
| `USER_ACTION_TOKEN_RETENTION_DAYS` | `7` | Used or expired email verification and password links. |
| `WORKSPACE_INVITATION_RETENTION_DAYS` | `30` | Expired or revoked invitations. Accepted invitations stay. |
| `AUDIT_EVENT_RETENTION_DAYS` | Unset | Audit log entries. Unset keeps the audit log forever. |
| `READ_NOTIFICATION_RETENTION_DAYS` | Unset | Read notifications. Unread notifications are never deleted. Unset keeps them forever. |

Ended sign-in and password-reset rate-limit windows are also deleted; they need no setting. Agent conversations, record status history and approvals, migration history and connector definitions are never deleted by cleanup.

Day counts must be positive integers. An invalid value stops startup.

## Object storage

Attricat stores uploaded files, generated image variants, extension artifacts, and presentation assets in one private S3-compatible bucket. All `S3_*` variables and the `s3_secret_access_key` [secret](#secrets) are required.

| Variable | Description |
| --- | --- |
| `S3_ENDPOINT` | Absolute HTTP(S) URL of the S3-compatible service. |
| `S3_REGION` | Signing region. |
| `S3_BUCKET` | Existing bucket name. Attricat does not create it in production. |
| `S3_ACCESS_KEY_ID` | Access key ID. Its secret key is the `s3_secret_access_key` secret. |
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
| `FILE_WORKER_OPERATIONS_BIND_ADDR` | `127.0.0.1:3001` | File worker | Private listener for `/health/live`, `/health/ready`, and `/metrics`. Outside loopback, `/metrics` requires the `file_worker_metrics_token` [secret](#secrets). |
| `FILE_WORKER_MAX_PIXELS` | `40000000` | File worker | Largest decoded image, in pixels, the worker will process. |
| `FILE_WORKER_MAX_ATTEMPTS` | `5` | File worker | Attempts before a processing job fails permanently. |
| `FILE_DELETE_GRACE_SECONDS` | `86400` (one day) | File worker | Time between an unreferenced file being marked deleted and its object being purged. |

A permanently failed processing job can be queued again. The attempt counter is reset only for a job that has failed permanently:

```sh
docker compose --env-file deploy/.env.production -f deploy/compose.yml run --rm file-worker file-worker --retry <job-uuid>
```

## Email

Attricat sends password-reset, invitation, and onboarding email over SMTP.

| Variable | Default | Description |
| --- | --- | --- |
| `SMTP_HOST` | `127.0.0.1` | SMTP server host. |
| `SMTP_PORT` | `1025` | SMTP server port. Normally `587` for `starttls` and `465` for `implicit`. |
| `SMTP_TLS_MODE` | `starttls` | `starttls` or `implicit`. `disabled` is accepted only for an unauthenticated local relay such as Mailpit; startup rejects credentials combined with `disabled`. |
| `SMTP_USERNAME` | Unset | SMTP username. Set both it and the `smtp_password` [secret](#secrets), or neither. |
| `MAIL_FROM` | `Attricat <no-reply@attricat.local>` | Sender address. |
| `PASSWORD_RESET_URL` | Local URL | Absolute URL of the web app's password-reset confirmation page, for example `https://attricat.example.com/password-reset/confirm`. |
| `WORKSPACE_INVITATION_URL` | Local URL | Absolute URL used in invitations to existing users, for example `https://attricat.example.com/invitations/accept`. |
| `WORKSPACE_ONBOARDING_URL` | Local URL | Absolute URL used in onboarding links for new users, for example `https://attricat.example.com/onboarding`. |

## Agents

Agents are off until the `llm_api_key` [secret](#secrets) is provided. The rest of the API starts normally without it. The provider must be compatible with the OpenAI Chat Completions API. See [Agents and approvals](/guides/agents/) for what data a provider receives.

| Variable | Default | Description |
| --- | --- | --- |
| `LLM_BASE_URL` | `https://api.openai.com/v1` | Absolute base URL of the Chat Completions API. |
| `LLM_MODEL` | `gpt-4o-mini` | Model identifier. Recorded on every run. |
| `LLM_REASONING_EFFORT` | Unset | Optional `reasoning_effort` sent with every request. Some models reject function tools unless this is `none`. |
| `LLM_REQUEST_TIMEOUT_SECONDS` | `60` | Timeout for one provider request, 1 to 3600. |
| `LLM_RUN_TIMEOUT_SECONDS` | `300` | Timeout for a whole agent run, 1 to 3600. |
| `LLM_MAX_TOOL_ROUNDS` | `25` | How many times an agent may call tools and continue before it stops, counted again after each approval, 1 to 100. |

These agent limits are fixed: 32 KiB per user message, 16 attachments per message, 5 MiB per inline image, eight tool-call rounds per run, 64 KiB per serialized tool result, and 32 tool calls per provider response.

## Extensions

| Variable | Default | Description |
| --- | --- | --- |
| `EXTENSION_OFFICIAL_REGISTRY` | `attricat/attricat-extensions` | GitHub `owner/repository` used as every workspace's built-in extension registry. |
| `EXTENSIONS_MODE` | `enabled` | Emergency switch. Set `disabled` to stop all extension execution, artifact delivery, commands, storage, host calls, and event delivery across the deployment. Installations and grants are left unchanged. Any other value stops startup. A change takes effect when the API restarts. |
| `EXTENSION_DENYLIST` | Empty | Comma-separated list of extension IDs (`acme.inventory`) or exact releases (`acme.inventory@<release-uuid>`) to block. Read at startup and checked on every runtime call. |
| `EXTENSION_PUBLIC_FILE_BASE_URL` | Unset | Externally reachable base URL of the public file links extensions issue, ending in `/api/public/extension-files/`, for example `https://attricat.example.com/api/public/extension-files/`. Marketplaces fetch listing images from it. It must be an HTTP(S) URL ending in `/`. Unset, extensions cannot issue file links. |
| `EXTENSION_OAUTH_CALLBACK_URL` | Unset | Externally reachable URL of the OAuth callback, ending in `/api/public/extension-oauth/callback`, for example `https://attricat.example.com/api/public/extension-oauth/callback`. Operators register it as their apps' redirect URI so authorization-code connections can be authorized in the browser. It must be served from the host operators sign in to Attricat at, because the callback only completes in the browser session that started the authorization and the browser sends that session's cookie to its own host only. A URL with an empty port, such as `http://127.0.0.1:/…`, stops startup. Unset, operators paste authorization codes by hand. |
| `EXTENSION_OAUTH_RETURN_URL` | `/manage/extensions/` on the callback's origin | Where the OAuth callback sends the browser afterwards: the web app's `/manage/extensions/`, ending in `/`. Set it when the web app is served from another origin than the API. |
| `EXTENSION_DEV_EGRESS_RESOLVE` | Unset | Development only; requires `ATTRICAT_DEVTOOLS=true`, otherwise startup stops. Comma-separated `host=ip:port` or `*.suffix=ip:port` entries that send extension requests for those hosts to a local mock, such as `*.mock.test=127.0.0.1:9443`. They bypass the public-address check, so never set them in production. |
| `EXTENSION_DEV_EGRESS_CA_FILE` | Unset | Development only; requires `ATTRICAT_DEVTOOLS=true`. A PEM file of certificates that extension requests trust in addition to the system roots, for a local mock's certificate. Never set it in production. |

## Web app development server

| Variable | Default | Description |
| --- | --- | --- |
| `ATTRICAT_API_URL` | `http://127.0.0.1:3000/api` | API base URL, including `/api`. The CLI uses it, and the Vite development server proxies `/api` to its origin. |
| `WEB_PORT` | `5173` | Port of the Vite development server. |

## Local development services

The repository's local stack runs PostgreSQL, Mailpit (email capture), Jaeger (traces), and RustFS (S3-compatible storage) in containers. These variables only set their host ports and credentials.

| Variable | Default |
| --- | --- |
| `POSTGRES_DB` | `attricat` |
| `POSTGRES_USER` | `postgres` |
| `POSTGRES_PASSWORD` | `postgres` |
| `POSTGRES_PORT` | `5432` |
| `MAILPIT_SMTP_PORT` | `1025` |
| `MAILPIT_UI_PORT` | `8025` |
| `JAEGER_OTLP_GRPC_PORT` | `4317` |
| `JAEGER_UI_PORT` | `16686` |
| `RUSTFS_PORT` | `9000` |
| `RUSTFS_CONSOLE_PORT` | `9001` |

`ATTRICAT_E2E_FIXTURE_EMAIL` and the `e2e_fixture_password` secret create a test user for end-to-end tests. Never set them in production.

In local development, `just setup` writes the secrets to the repository's ignored `.secrets` directory and points `ATTRICAT_SECRETS_DIR` at it.

## CLI

The `acli` command-line client reads its own variables. See the [CLI reference](/reference/cli/#configuration).

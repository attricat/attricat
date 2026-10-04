# Configuration Reference

The API loads `.env` from the working directory at startup. Run `just setup`
once before `just dev`; it creates `.env` from `.env.example` and assigns
persistent, worktree-specific ports. Those port assignments, plus ready-to-open
`WEB_URL`, `MAILPIT_UI_URL`, `JAEGER_UI_URL`, and `RUSTFS_UI_URL` values, are recorded in the ignored
`.worktree` file. The API and file worker validate object-storage
configuration and bucket access during startup; neither starts with a missing
or inaccessible configured bucket.

| Setting | Default | Used by | Purpose |
| --- | --- | --- | --- |
| `DATABASE_URL` | Required | API and SQLx | PostgreSQL connection string. |
| `DATABASE_REQUEST_POOL_CONNECTIONS` | `10` | API | Maximum connections in the single global request/session pool shared by all workspaces. Must be an integer from 1 to 100. |
| `DATABASE_TASK_POOL_CONNECTIONS` | `10` | API | Maximum connections in the single global task/worker pool shared by all workspaces. Must be an integer from 1 to 100. The API logs the request + task + maintenance total at startup. Background coordinators hold up to three further connections outside both pools; see [Coordinator leadership](#coordinator-leadership). |
| `BIND_ADDR` | `127.0.0.1:3000` | API | Listener address. The production image sets `0.0.0.0:3000`. |
| `CATALOG_AUTO_MIGRATE` | `true` | API | Applies embedded migrations during API startup. Production deployment sets this `false` and runs the image's singleton `migrate` role first. |
| `WEB_DIST_DIR` | Unset | API | Optional compiled SPA directory. The production image sets `/srv/attricat/web`; API routes are then also available below `/api`. |
| `CATALOG_WORKSPACE_ID` | Bootstrap `default` workspace UUID | API | Workspace initialized with the configured owner during startup; it is not an HTTP tenancy selector. |
| `CATALOG_BOOTSTRAP_WORKSPACE_NAME` | `Default workspace` | API | Display name recorded while initializing the configured workspace. |
| `CATALOG_BOOTSTRAP_OWNER_EMAIL` | `owner@example.test` | API | Initial owner email. Startup trims and lowercases it before idempotently creating the bootstrap user, membership, and owner grant. Set a real deployment email; it is never an API input. |
| `CATALOG_BOOTSTRAP_OWNER_ID` | Random UUID | API | Optional stable UUID for the bootstrap owner. |
| `CATALOG_BOOTSTRAP_OWNER_PASSWORD` | Unset | API | Optional one-time local password for a newly bootstrapped owner. It is hashed before persistence and never updates an existing credential. |
| `SESSION_COOKIE_SECURE` | `true` | API | Adds `Secure` to browser session and CSRF cookies. Set `false` only for local HTTP development or test servers. |
| `RUST_LOG` | `info` | API | Structured tracing filter (for example, `api=debug`). |
| `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` | Unset | API and file worker | Optional OTLP/gRPC trace collector endpoint. Local development sends traces to Jaeger. Unset it to disable trace export. |
| `CATALOG_DEVTOOLS` | `true` locally | API process and Vite | Shared switch for development tooling. It controls the web Inspector and sanitized Explorer `Server-Timing` metrics, including aggregate SQL execution time and query count; these Explorer metrics are absent when disabled. Other builds, including production, load the Inspector on demand only in a browser where local storage `catalog.inspector-enabled` is `true`; it then shows server status, the running build, and the session, but no Explorer timings unless the API also enables this setting. SQL statements and bind values are never exposed. |
| `LLM_API_KEY` | Unset (agents unavailable) | API only | Secret API key for the OpenAI-compatible provider. Never send, persist, or log it. |
| `LLM_BASE_URL` | `https://api.openai.com/v1` | API only | Absolute HTTP(S) base URL for OpenAI-compatible Chat Completions. |
| `LLM_MODEL` | `gpt-4o-mini` | API only | Provider model identifier captured on each run, never a browser-selected setting. |
| `LLM_REASONING_EFFORT` | Unset | API only | Optional Chat Completions `reasoning_effort` value sent on every agent provider request when configured. Use a value supported by your provider/model (for example, `none` for models that reject function tools with reasoning enabled); omitted by default for providers that do not support it. This does not switch the adapter to the Responses API. |
| `LLM_REQUEST_TIMEOUT_SECONDS` | `60` | API only | Per-provider-request timeout, 1–3600 seconds. |
| `LLM_RUN_TIMEOUT_SECONDS` | `300` | API only | Total agent-run timeout, 1–3600 seconds. |
| `PREVIEW_MAX_RELATIONSHIP_DEPTH` | `3` | API | Maximum recursive relationship preview depth. |
| `PREVIEW_MAX_RELATIONSHIP_ITEMS` | `10` | API | Maximum inline targets per relationship. |
| `ENTITY_MAX_PAGE_SIZE` | `100` | API | Maximum page size for relationship browsing. |
| `DATA_HEALTH_CACHE_TTL_SECONDS` | `300` | API | Data-health response cache lifetime. Any recorded catalog change also makes the next request a miss. |
| `CACHE_BACKEND` | `memory` | API | Query cache backend: `memory` keeps cached definitions in each process; `redis` also shares cached values and extension network rate limits between replicas. Nothing is invalidated through Redis: correctness never depends on the backend (see [Caching](caching.md)). An unreachable Redis degrades to memory while the API reconnects in the background. |
| `REDIS_URL` | Unset | API | Redis URL, required when `CACHE_BACKEND=redis`. `redis://` and TLS `rediss://` (rustls, platform root certificates) are supported. `just setup` writes the local development Redis URL. |
| `CACHE_MAX_ENTRIES` | `20000` | API | Positive maximum number of in-memory query cache entries per process. |
| `CACHE_KEY_PREFIX` | `attricat` | API | First segment of every Redis key, followed by the database's random identity. Change it, or flush Redis, after restoring a database backup. Must not contain whitespace. |
| `INCOMING_RELATIONSHIP_MAX_PAGE_SIZE` | `50` | API | Maximum page size for incoming-relationship browsing. |
| `RELATIONSHIP_FACET_MAX_NODES` | `100` | API | Maximum relationship nodes considered while building Explorer facets. |
| `ATTRIBUTE_VALUE_HISTORY_RETENTION_DAYS` | `90` | API | Number of days of attribute-value history retained; must be a positive signed 64-bit integer. Invalid values stop startup. A periodic worker deletes at most 1,000 rows per transaction, with a 10-second sweep budget every minute. Failures are logged and retried without blocking startup. |
| `HTTP_REQUEST_TIMEOUT_SECONDS` | `30` | API | Positive wall-clock limit through response creation. Timed-out requests return `503` with code `request_timeout`; mutations must not be blindly replayed. |
| `HTTP_MAX_CONCURRENT_REQUESTS` | `256` | API | Positive process-local in-flight handler cap. Excess requests return `503`. Health probes have independent admission; event-stream bodies have separate limits. |
| `HTTP_MAX_EVENT_STREAMS` | `128` | API | Positive process-local cap held for each SSE response body's lifetime. |
| `HTTP_MAX_EVENT_STREAMS_PER_PRINCIPAL` | `4` | API | Positive concurrent SSE cap per workspace/user, shared across that user's tokens and sessions. |
| `HTTP_EVENT_STREAM_LIFETIME_SECONDS` | `900` | API | Positive maximum SSE lifetime. Clients reconnect using `Last-Event-ID`; shutdown also closes streams. |
| `HTTP_DEFAULT_BODY_BYTES` | `2097152` | API | Positive default body limit. Streaming upload routes explicitly disable it and enforce their file-specific limits. |
| `EVENT_DISPATCHER_LEASE_SECONDS` | `30` | API | Positive lease duration for one event-handler attempt. A shorter lease raises duplicate-delivery risk. |
| `EVENT_DISPATCHER_RETRY_INITIAL_SECONDS` | `1` | API | Positive initial failed-delivery retry delay. |
| `EVENT_DISPATCHER_RETRY_MAX_SECONDS` | `60` | API | Positive cap on exponential failed-delivery retry delay. |
| `EVENT_DISPATCHER_MAX_ATTEMPTS` | `5` | API | Positive number of claims before a failed delivery becomes `dead_letter`. |
| `EVENT_DISPATCHER_POLL_MILLIS` | `250` | API | Positive delay between dispatcher polls. |
| `TASK_WORKER_ID` | Random process UUID | API | Stable 1–128-byte identity for the supervised generic task worker. |
| `TASK_WORKER_CONCURRENCY` | `8` | API | Positive maximum number of registered generic-task handlers running at once. |
| `TASK_WORKER_POLL_MILLIS` | `250` | API | Positive idle delay for the generic task worker. |
| `TASK_WORKER_SHUTDOWN_GRACE_SECONDS` | `30` | API | Positive bounded drain period; unfinished generic task leases are allowed to expire. |
| `BLUEPRINT_MIGRATION_PAGE_SIZE` | `100` | API | Entity migration candidate keyset page size; must be between 1 and 1000. |
| `BLUEPRINT_MIGRATION_CONCURRENCY` | `4` | API | Maximum concurrent entity attempts within one migration batch; must be between 1 and 64. |
| `CATALOG_API_URL` | `http://127.0.0.1:3000` | Vite | API target for the web app's `/api` development proxy. |
| `EXTENSION_OFFICIAL_REGISTRY` | `attricat/attricat-extensions` | API | Canonical public GitHub `owner/repository` used as every workspace's immutable official extension source. |
| `EXTENSIONS_MODE` | `enabled` | API | Deployment emergency gate. Set exactly `disabled` to block all new extension execution, artifacts, runtime descriptors, commands, storage, host calls, and event delivery without changing installations or grants. Invalid configured values fail closed. |
| `EXTENSION_DENYLIST` | Empty | API | Comma-separated targeted containment entries. Each entry is an extension ID (`acme.extension`) or exact release (`acme.extension@uuid`). Evaluated at every runtime gate. |
| `POSTGRES_DB` | `catalog` | Docker Compose | Local PostgreSQL database name. |
| `POSTGRES_USER` | `postgres` | Docker Compose | Local PostgreSQL user. |
| `POSTGRES_PASSWORD` | `postgres` | Docker Compose | Local PostgreSQL password. |
| `POSTGRES_PORT` | `5432` | Docker Compose | Host port mapped to PostgreSQL. |
| `WEB_PORT` | `5173` | Vite | Listener port for the development web app. |
| `SMTP_HOST` | `127.0.0.1` | API local development | Mailpit SMTP host. |
| `SMTP_PORT` | `1025` | API local development | Mailpit SMTP port; `just setup` sets it to the worktree-specific port. |
| `SMTP_TLS_MODE` | `starttls` | API | Required SMTP encryption mode: `starttls` or `implicit` in production. `disabled` is only for the trusted local Mailpit relay. Opportunistic TLS is rejected. |
| `SMTP_USERNAME` | Unset | API | Optional SMTP username. |
| `SMTP_PASSWORD` | Unset | API | Optional SMTP password; keep it in a secret manager outside local development. |
| `MAIL_FROM` | `Catalog <no-reply@catalog.local>` | API local development | Sender address for lifecycle email. |
| `PASSWORD_RESET_URL` | Local web confirmation URL | API local development | Absolute web URL used in reset email; `just setup` uses the worktree's `WEB_PORT`. |
| `WORKSPACE_INVITATION_URL` | Local invitation URL | API local development | Absolute web URL used for delivered existing-user workspace invitations. |
| `WORKSPACE_ONBOARDING_URL` | Local onboarding URL | API local development | Absolute web URL used for delivered new-user onboarding links. |
| `MAILPIT_SMTP_PORT` | `1025` | Docker Compose | Worktree-specific host port mapped to Mailpit SMTP. |
| `MAILPIT_UI_PORT` | `8025` | Docker Compose | Worktree-specific host port for Mailpit's UI and REST API. |
| `JAEGER_OTLP_GRPC_PORT` | `4317` | Docker Compose | Worktree-specific host port for Jaeger's OTLP/gRPC receiver. |
| `JAEGER_UI_PORT` | `16686` | Docker Compose | Worktree-specific host port for Jaeger's trace UI. |
| `S3_ENDPOINT` | Required | API and file worker | Absolute HTTP(S) URL for the S3-compatible endpoint. Local development uses RustFS. |
| `S3_REGION` | Required | API and file worker | S3 signing region. |
| `S3_BUCKET` | Required | API and file worker | Existing bucket used for catalog objects. Local startup creates it. |
| `S3_ACCESS_KEY_ID` | Required | API and file worker | S3 access key. |
| `S3_SECRET_ACCESS_KEY` | Required | API and file worker | S3 secret access key. Do not log or expose it. |
| `S3_FORCE_PATH_STYLE` | Required | API and file worker | Strict `true`/`false` setting for S3 path-style addressing. Set `true` for local RustFS. |
| `S3_UPLOAD_TIMEOUT_SECONDS` | Required | API and file worker | Positive timeout for object uploads and deletes. |
| `S3_DOWNLOAD_TIMEOUT_SECONDS` | Required | API and file worker | Positive timeout for object downloads and bucket readiness. |
| `FILE_UPLOAD_MAX_BYTES` | `52428800` | API | Positive request-level byte limit for each streamed upload. A file attribute may set a lower `max_bytes` policy. |
| `FILE_UPLOAD_MAX_FILES` | `10` | API | Positive request-level number of file parts. A `cardinality = "one"` attribute accepts exactly one. |
| `FILE_WORKER_ID` | Random process UUID | File worker | Stable identifier written with claimed jobs. |
| `FILE_WORKER_POLL_MILLISECONDS` | `500` | File worker | Delay between durable-job polls. Queue metrics are refreshed during idle polls. |
| `FILE_WORKER_OPERATIONS_BIND_ADDR` | `127.0.0.1:3001` | File worker | Private listener for `/health/live`, dependency-aware `/health/ready`, and `/metrics`. |
| `FILE_WORKER_METRICS_TOKEN` | Unset on loopback | File worker | Bearer token for `/metrics`; required at startup when the operations listener is not loopback. |
| `FILE_WORKER_MAX_PIXELS` | `40000000` | File worker | Maximum decoded image pixels accepted for processing. |
| `FILE_WORKER_MAX_ATTEMPTS` | `5` | File worker | Attempts before a job becomes terminally failed. |
| `FILE_DELETE_GRACE_SECONDS` | `86400` | File worker | Delay between an unreferenced file being soft-deleted and its object purge. |
| `CATALOG_E2E_FIXTURE_EMAIL` | Unset | API test environments | Optional test fixture user email. It takes effect only when paired with `CATALOG_E2E_FIXTURE_PASSWORD`; do not set either in production. |
| `CATALOG_E2E_FIXTURE_PASSWORD` | Unset | API test environments | Optional test fixture user password paired with `CATALOG_E2E_FIXTURE_EMAIL`; do not set either in production. |
| `RUSTFS_PORT` | `9000` | Docker Compose | Worktree-specific host port for the local RustFS S3 API. |
| `RUSTFS_CONSOLE_PORT` | `9001` | Docker Compose | Worktree-specific host port for the local RustFS console. |
| `REDIS_PORT` | `6379` | Docker Compose | Worktree-specific host port for the local Redis that `just dev` starts. The API uses it only with `CACHE_BACKEND=redis`. |

## Coordinator leadership

The rule-schedule, workflow-schedule and extension-intake coordinators each run
on one API replica at a time. Their sweeps are idempotent, so this saves work
rather than guarding correctness. The leader holds a PostgreSQL session
advisory lock on a dedicated connection opened outside the request, task and
maintenance pools; the lock is released when that connection closes, including
when the process dies.

- Each API process may hold up to three extra connections, one per coordinator
  it leads. Include them when sizing `max_connections`.
- A non-leader opens a short-lived connection every 5 seconds per coordinator
  to try the lock, so a replacement leader takes over within about 5 seconds.
  The leader checks its lock connection every 30 seconds.
- Session advisory locks need a session-pinned connection. Point
  `DATABASE_URL` at PostgreSQL directly or at a pooler in session mode;
  PgBouncer transaction pooling is not supported.

## Durable API task queue

The `tasks` table is the durable delivery envelope for the API-owned task queue. Its
initial closed registry is reserved for agent runs, domain-event deliveries, workflow
runs, rule runs, and blueprint migration batches. Task payloads are small reference
objects only; diagnostics intentionally exclude them. Agent runs, extension-event
deliveries, workflow runs, rule runs, and blueprint migration batches are all executed
by the supervised worker. On shutdown it stops claiming, lets registered handlers
heartbeat through the configured grace period, then allows unfinished leases to expire.
File processing is excluded and continues to use its separate file-worker configuration
and `file_processing_jobs` table.

## Agent provider

Agents are disabled when `LLM_API_KEY` is absent or blank; ordinary catalog API
startup continues. Set the key, base URL, and model only in the API process
environment (or its secret manager), never in browser configuration or source
control. The provider is OpenAI Chat-Completions compatible. The API validates
the URL and bounded timeouts at startup, logs only whether agents are
available, and stores provider base URL/model snapshots—not credentials—in
durable agent records.

An enabled provider receives the conversation history and bounded catalogue
results needed to answer it, including attachments as described below. Choose a
provider and retention policy suitable for that data. Agent access requires the
`agents.run` permission. Read-only tools execute automatically; all writes
pause for a durable human approval. Approval is not a
permission bypass: the initiating user is re-authorized when an approved write
resumes. Restrict this permission to trusted operators and review each proposed
input and change summary.

### macOS LiteLLM loopback proxy

On macOS, local-network privacy or firewall policy can allow `curl` to reach a
LAN-hosted LiteLLM instance while denying the Rust API binary with `No route to
host`. This is a host networking issue, not an invalid `LLM_BASE_URL`. For
local development, route the API through the included Node-based loopback proxy
instead:

```sh
# .env
LLM_UPSTREAM_BASE_URL=http://192.168.1.100:4000/v1
LLM_LOOPBACK_PROXY_PORT=4010
LLM_BASE_URL=http://127.0.0.1:4010/v1

# In another terminal, alongside the development stack
set -a; source .env; set +a
node scripts/litellm-loopback-proxy.mjs
```

The proxy listens only on `127.0.0.1`, forwards streaming Chat Completions
requests unchanged to `LLM_UPSTREAM_BASE_URL`, and does not log provider
credentials or request bodies. Keep it running while the API is running. Fix
the host networking policy instead of using this development workaround in a
deployed environment.

### Attachment forwarding

Every provider request contains the complete stored conversation history, so an
attachment is considered again on every run and tool-call round while its
message remains in that history. For each conversation-message attachment, the
provider always receives a text record containing its display filename, MIME
type, and Catalog file ID. It does **not** receive the object-store key, a
signed download URL, checksum, or byte size.

The original file bytes are forwarded only when the attachment's MIME type
starts with `image/` and both its recorded size and bytes read from private
object storage are at most 5 MiB (5,242,880 bytes). Those bytes are sent as a
base64 `data:` image URL alongside the text record. Non-image attachments,
images over the limit, and images that cannot be read are not sent as file
content; they remain represented by the filename, MIME type, and file ID (an
unreadable eligible image is additionally marked as unreadable in the text
record). The 5 MiB threshold is fixed, not configurable. A message accepts at
most 16 attachments.

The provider can also request a workspace file through a read-only tool. A
successful `view_image` tool result sends an image display variant, or the
original if no display variant is available, only when it is at most 1 MiB;
otherwise it sends the tool result metadata without image bytes. A successful
`read_file` result sends UTF-8 file text only when it is at most 64 KiB;
otherwise it sends its result metadata without file text. These tool results
include the requested file ID, filename, and MIME type.

Catalog retains conversation messages and their attachment records (file ID,
display filename, MIME type, byte size, and status) and keeps referenced file
objects according to the file-retention policy below. It does not retain raw
provider response bodies or provider credentials. The configured provider
receives the transmitted content; whether it logs, retains, or uses that data
is governed by that provider's account, API terms, and retention controls.

Agent safety limits are fixed in the API's shared agent configuration: a 32 KiB
user message, 512-byte conversation title and model name, 16 attachments per
message, 5 MiB inline image attachment, 64 KiB serialized tool result, eight
tool-call rounds per run, 64 KiB total provider response and undrained SSE
frame buffers, 32 KiB assistant text, 16 KiB tool arguments, and 32 provider
tool calls per response. The maximum provider request/run timeout remains one
hour. Provider failures, malformed responses, unknown tools, invalid tool arguments,
and tool-round exhaustion are recorded as failed durable runs; provider
response bodies and credentials are not retained. Schedules use six-field UTC cron expressions.
An occurrence that overlaps a queued, running, or approval-waiting run is
recorded as skipped rather than executed concurrently.

## Local mail and object storage

Mailpit is a local-development and E2E adapter only; it is not production mail
configuration. Source `.worktree` after `just dev`, open
`$MAILPIT_UI_URL` for manual inspection, and use its REST API
for E2E mailbox retrieval. Production SMTP requires `SMTP_TLS_MODE=starttls`
or `implicit`; `disabled` is restricted to an unauthenticated trusted local relay. Startup rejects credentials with `disabled` and also rejects partial username/password configuration. See
[Production operations](operations.md) for rollout, rotation, and recovery.

A failed durable job can be returned to the queue by an operator with
`cargo run -p api --bin file-worker -- --retry <job-uuid>`. The command resets
its attempt counter only when the job is terminally failed.

RustFS is a local-development S3-compatible adapter, replacing the deprecated
MinIO local stack. It is not a production provider selection: production uses
the same generic `S3_*` settings for its chosen S3-compatible service. The
local bucket is initialized as `catalog-files`; open `$RUSTFS_UI_URL` after
sourcing `.worktree` to inspect it. Back up PostgreSQL file metadata
and the configured bucket together once file uploads are enabled.

## File storage operations

The API process streams multipart parts to private temporary files, verifies the
file signature and immutable blueprint policy, then streams the staged object to
S3. It does not return object-store URLs or accept object keys from clients.
The file worker reads originals for processing, writes generated variants, and
deletes objects. Authorized downloads through the API also read originals and
variants. Give API and worker credentials
only the minimum bucket permissions (`PutObject`, `GetObject`, `DeleteObject`,
and `HeadBucket`); keep the bucket private and terminate TLS at the S3 endpoint.

Uploaded files are retained while referenced. Conversation uploads have a
15-minute pending-attachment window, so reconciliation does not delete them
between the upload response and message creation. Attaching a file clears that
window in the same transaction as its attachment record. Never-attached uploads
become ordinary unreferenced files after the window expires. Reconciliation then
marks them deleted and schedules their purge after `FILE_DELETE_GRACE_SECONDS`
(one day by default). This is a grace period, not a backup policy: restore a
file by recreating its reference before the purge is claimed. Back up and
restore the PostgreSQL database and the S3 bucket as one consistent unit.
Restoring only one can leave metadata without objects or orphaned objects; run
the worker afterwards to reconcile the restored state.

RustFS is the supported local and E2E S3-compatible adapter. It is deliberately
configured through the same AWS SDK and `S3_*` settings used in production, so
provider compatibility is exercised without making MinIO a development
requirement.

## Request authorization

Most catalog API routes require either an active browser session or a personal
API token with the relevant permission. Public routes include the health probes,
authentication discovery/login, password reset, and onboarding completion;
private monitoring routes have their own access rules. The API checks the
credential's workspace and grants for each request; missing or invalid
credentials return `401`, while a valid identity without the required grant
returns `403`. Unsafe **cookie-authenticated** requests also require the
`X-Catalog-Csrf` synchronizer token.

Browser sessions are scoped to the workspace resolved from the submitted login
identifier. Clients do not provide a workspace UUID or tenancy header.

The CLI resolves its API URL in this order: `--server`, `CATALOG_SERVER`,
`CATALOG_API_URL` from the environment or worktree `.env`, then
`http://127.0.0.1:3000`. See [the CLI guide](cli.md#start-locally).

The API validates the five `EVENT_DISPATCHER_*` settings above during startup;
zero, non-integer, or an unrepresentable `EVENT_DISPATCHER_MAX_ATTEMPTS` stops
startup. They tune the internal at-least-once event dispatcher only. See
[Domain eventing](eventing.md) for delivery, retry, and operator behavior.

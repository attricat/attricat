# Configuration Reference

The API loads `.env` from the working directory at startup. `just setup` and
`just dev` create it from `.env.example` with persistent, worktree-specific
ports. Those port assignments, plus ready-to-open `WEB_URL` and
`MAILPIT_UI_URL`, and `RUSTFS_UI_URL` values, are recorded in the ignored
`.catalog-worktree` file. The API and file worker validate object-storage
configuration and bucket access during startup; neither starts with a missing
or inaccessible configured bucket.

| Setting | Default | Used by | Purpose |
| --- | --- | --- | --- |
| `DATABASE_URL` | Required | API and SQLx | PostgreSQL connection string. |
| `BIND_ADDR` | `127.0.0.1:3000` | API | Listener address. |
| `CATALOG_WORKSPACE_ID` | Bootstrap `default` workspace UUID | API | Workspace initialized with the configured owner during startup; it is not an HTTP tenancy selector. |
| `CATALOG_BOOTSTRAP_WORKSPACE_NAME` | `Default workspace` | API | Display name recorded while initializing the configured workspace. |
| `CATALOG_BOOTSTRAP_OWNER_EMAIL` | `owner@example.test` | API | Initial owner email. Startup trims and lowercases it before idempotently creating the bootstrap user, membership, and owner grant. Set a real deployment email; it is never an API input. |
| `CATALOG_BOOTSTRAP_OWNER_ID` | Random UUID | API | Optional stable UUID for the bootstrap owner. |
| `CATALOG_BOOTSTRAP_OWNER_PASSWORD` | Unset | API | Optional one-time local password for a newly bootstrapped owner. It is hashed before persistence and never updates an existing credential. |
| `SESSION_COOKIE_SECURE` | `true` | API | Adds `Secure` to browser session and CSRF cookies. Set `false` only for local HTTP development or test servers. |
| `RUST_LOG` | `info` | API | Structured tracing filter (for example, `api=debug`). |
| `LLM_API_KEY` | Unset (agents unavailable) | API only | Secret API key for the OpenAI-compatible provider. Never send, persist, or log it. |
| `LLM_BASE_URL` | `https://api.openai.com/v1` | API only | Absolute HTTP(S) base URL for OpenAI-compatible Chat Completions. |
| `LLM_MODEL` | `gpt-4o-mini` | API only | Provider model identifier captured on each run, never a browser-selected setting. |
| `LLM_REQUEST_TIMEOUT_SECONDS` | `60` | API only | Per-provider-request timeout, 1–3600 seconds. |
| `LLM_RUN_TIMEOUT_SECONDS` | `300` | API only | Total agent-run timeout, 1–3600 seconds. |
| `AGENT_SCHEDULER_POLL_SECONDS` | `15` | API only | Durable schedule-worker polling interval, 1–3600 seconds. |
| `PREVIEW_MAX_RELATIONSHIP_DEPTH` | `3` | API | Maximum recursive relationship preview depth. |
| `PREVIEW_MAX_RELATIONSHIP_ITEMS` | `10` | API | Maximum inline targets per relationship. |
| `ENTITY_MAX_PAGE_SIZE` | `100` | API | Maximum page size for relationship browsing. |
| `DATA_HEALTH_CACHE_TTL_SECONDS` | `300` | API | Data-health response cache lifetime. |
| `CATALOG_API_URL` | `http://127.0.0.1:3000` | Vite | API target for the web app's `/api` development proxy. |
| `POSTGRES_DB` | `catalog` | Docker Compose | Local PostgreSQL database name. |
| `POSTGRES_USER` | `postgres` | Docker Compose | Local PostgreSQL user. |
| `POSTGRES_PASSWORD` | `postgres` | Docker Compose | Local PostgreSQL password. |
| `POSTGRES_PORT` | `5432` | Docker Compose | Host port mapped to PostgreSQL. |
| `WEB_PORT` | `5173` | Vite | Listener port for the development web app. |
| `SMTP_HOST` | `127.0.0.1` | API local development | Mailpit SMTP host. |
| `SMTP_PORT` | `1025` | API local development | Mailpit SMTP port; `just setup` sets it to the worktree-specific port. |
| `MAIL_FROM` | `Catalog <no-reply@catalog.local>` | API local development | Sender address for lifecycle email. |
| `PASSWORD_RESET_URL` | Local web confirmation URL | API local development | Absolute web URL used in reset email; `just setup` uses the worktree's `WEB_PORT`. |
| `WORKSPACE_INVITATION_URL` | Local invitation URL | API local development | Absolute web URL used for delivered existing-user workspace invitations. |
| `WORKSPACE_ONBOARDING_URL` | Local onboarding URL | API local development | Absolute web URL used for delivered new-user onboarding links. |
| `MAILPIT_SMTP_PORT` | `1025` | Docker Compose | Worktree-specific host port mapped to Mailpit SMTP. |
| `MAILPIT_UI_PORT` | `8025` | Docker Compose | Worktree-specific host port for Mailpit's UI and REST API. |
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
| `FILE_WORKER_POLL_MILLISECONDS` | `500` | File worker | Delay between durable-job polls. |
| `FILE_WORKER_MAX_PIXELS` | `40000000` | File worker | Maximum decoded image pixels accepted for processing. |
| `FILE_WORKER_MAX_ATTEMPTS` | `5` | File worker | Attempts before a job becomes terminally failed. |
| `FILE_DELETE_GRACE_SECONDS` | `86400` | File worker | Delay between an unreferenced file being soft-deleted and its object purge. |
| `RUSTFS_PORT` | `9000` | Docker Compose | Worktree-specific host port for the local RustFS S3 API. |
| `RUSTFS_CONSOLE_PORT` | `9001` | Docker Compose | Worktree-specific host port for the local RustFS console. |

## Agent provider

Agents are disabled when `LLM_API_KEY` is absent or blank; ordinary catalog API
startup continues. Set the key, base URL, and model only in the API process
environment (or its secret manager). The provider is OpenAI Chat-Completions
compatible. The API validates the URL and bounded timeouts at startup, logs only
whether agents are available, and stores provider base URL/model snapshots—not
credentials—in durable agent records.

Mailpit is a local-development and E2E adapter only; it is not production mail
configuration. Source `.catalog-worktree` after `just dev`, open
`$MAILPIT_UI_URL` for manual inspection, and use its REST API
for E2E mailbox retrieval. Production mail delivery is deliberately deferred.

A failed durable job can be returned to the queue by an operator with
`cargo run -p api --bin file-worker -- --retry <job-uuid>`. The command resets
its attempt counter only when the job is terminally failed.

RustFS is a local-development S3-compatible adapter, replacing the deprecated
MinIO local stack. It is not a production provider selection: production uses
the same generic `S3_*` settings for its chosen S3-compatible service. The
local bucket is initialized as `catalog-files`; open `$RUSTFS_UI_URL` after
sourcing `.catalog-worktree` to inspect it. Back up PostgreSQL file metadata
and the configured bucket together once file uploads are enabled.

## File storage operations

The API process streams multipart parts to private temporary files, verifies the
file signature and immutable blueprint policy, then streams the staged object to
S3. It does not return object-store URLs or accept object keys from clients.
The file worker is the only component that reads originals for processing,
writes generated variants, or deletes objects. Give API and worker credentials
only the minimum bucket permissions (`PutObject`, `GetObject`, `DeleteObject`,
and `HeadBucket`); keep the bucket private and terminate TLS at the S3 endpoint.

Uploaded files are retained while referenced. Reconciliation marks an
unreferenced file deleted and schedules its purge after
`FILE_DELETE_GRACE_SECONDS` (one day by default). This is a grace period, not a
backup policy: restore a file by recreating its reference before the purge is
claimed. Back up and restore the PostgreSQL database and the S3 bucket as one
consistent unit. Restoring only one can leave metadata without objects or
orphaned objects; run the worker afterwards to reconcile the restored state.

RustFS is the supported local and E2E S3-compatible adapter. It is deliberately
configured through the same AWS SDK and `S3_*` settings used in production, so
provider compatibility is exercised without making MinIO a development
requirement.

## Request authorization

All catalog API routes except `/health`, `POST /auth/discover`, and `POST /auth/login` require an
active browser session cookie. The API verifies its active membership and role
grant for each request; absent or invalid sessions are `401`, while a valid
identity without a matching grant is `403`. Unsafe requests must additionally
provide the `X-Catalog-Csrf` synchronizer token.

Browser sessions are scoped to the workspace resolved from the submitted login
identifier. Clients do not provide a workspace UUID or tenancy header.

`CATALOG_SERVER` overrides the CLI's API URL. The CLI otherwise targets
`http://127.0.0.1:3000`.

`AGENT_SCHEDULER_POLL_SECONDS` controls the API process UTC schedule poll
interval (default `15`, range `1`–`3600`). Queued runs are recovered when the
API process starts; no external scheduler is required.

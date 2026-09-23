# Catalog

Catalog is a versioned catalog engine with a Rust API and CLI plus a React web
application. Blueprints define versioned entity schemas; entities retain the
exact revision they were created with.

## Run Locally

Install a Docker-compatible container runtime, Rust, Node.js 18 or newer,
`just`, `process-compose`, and `watchexec`. Then, from the repository root:

```sh
just setup
just dev
```

Run `just setup` once before any other `just` recipe. It assigns persistent,
worktree-specific ports, writes them and ready-to-open `WEB_URL`, `DOCS_URL`,
`MAILPIT_UI_URL`, `JAEGER_UI_URL`, and `RUSTFS_UI_URL` values to the ignored
`.worktree` file, and creates `.env` from `.env.example`. `just dev`
starts PostgreSQL, Mailpit, Jaeger, and RustFS (the local S3-compatible object store)
before starting the API, file worker, web app, and public documentation site.
Stop `just dev` with `Ctrl-C`; its exit trap also runs `just down`, stopping
and removing the local containers.

For database debugging, `just sql` opens an interactive `psql` session inside
this worktree's PostgreSQL container.

See [Getting Started](docs/index.md#getting-started) for database, migration,
and test instructions. The public docs site is available at `$DOCS_URL`; source
`.worktree` and open it after starting `just dev`. Mailpit also starts
with the local stack; open `$MAILPIT_UI_URL` to inspect local email. Jaeger receives
API and file-worker traces; open `$JAEGER_UI_URL` to inspect them.

## Sign in locally

Open `/login` and enter the bootstrap workspace identifier `default.local`, then
select **Continue**. Sign in with the bootstrap-owner email configured in `.env`
(`CATALOG_BOOTSTRAP_OWNER_EMAIL`) and its password. The workspace UUID is an
internal database/configuration identifier; clients do not select it directly.

## Conversational agents

Agents are optional. Set `LLM_API_KEY` in the API process environment (and, if
needed, `LLM_BASE_URL` and `LLM_MODEL`), restart the API, then open
**Conversations** in the web application. The browser never receives the
provider credential; an absent or blank key disables agent runs without
preventing normal catalog use.

Treat an enabled provider as a trusted operator integration: conversation
content and tool results are sent to the configured provider. Read-only tools
run automatically, but every catalog mutation—interactive or scheduled—stops
for a durable, explicit approval. An approved scheduled run is re-authorized
as its initiating user before it writes. Review the proposed arguments and
change summary, use a least-privileged provider account, and only grant
`agents.run` to users allowed to request catalog changes. See the
[agent configuration](docs/configuration.md#agent-provider) and
[agent API contract](docs/api.md#agents) for limits and operational behavior.

## Production image

Attricat ships as one immutable image with `migrate`, `api`, and `file-worker`
commands. The API command also serves the compiled web UI; PostgreSQL, private
S3-compatible storage, SMTP, and monitoring remain external services. See
[`deploy/compose.yml`](deploy/compose.yml), the matching production environment
example, and [Production operations](docs/operations.md) for rollout, backup,
restore, and rollback.

## Documentation

- Public documentation site: `apps/docs` (run with `pnpm --dir apps/docs dev`)
- [Documentation index](docs/index.md)
- [Blueprint authoring](docs/blueprints.md)
- [Extension development and local side-loading](docs/extensions.md#local-extension-integration-testing)
- [Catalog CLI](docs/cli.md)
- [API reference](docs/api.md)
- [Configuration reference](docs/configuration.md)
- [Database model](docs/database.md)
- [File storage, worker, and retention configuration](docs/configuration.md#file-storage-operations)
- [Local account lifecycle](docs/authentication.md)
- [Demo catalog generator](examples/generate.md)
- [Relationships walkthrough](examples/relationships/README.md)

## Applications

- `apps/api`: Axum API and SQLx migrations.
- `apps/catalog-cli`: JSON-first HTTP command-line client.
- `apps/catalog-web`: React and Vite web application.

## Database policy

Database migrations are declarative only: schema objects, columns, indexes,
foreign keys, `NOT NULL`, `UNIQUE`, and `CHECK` constraints, and required types
or extensions. Do not add SQL functions, procedures, triggers, views, RLS
policies, or `DO` blocks. Authorization, validation, state transitions,
rate-limiting, auditing, retention, and all other behavior belong in the Rust
application and must be performed through repository transactions.

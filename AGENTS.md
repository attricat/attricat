# Frontend

Follow [Frontend Conventions](docs/frontend.md) for `apps/catalog-web`.

## Attricat design system

The [design repository](https://github.com/attricat/design) defines Attricat's
visual identity. A pinned copy of its tokens and brand assets is stored in
[`apps/catalog-web/design/`](apps/catalog-web/design/) for reproducible builds;
see its README for the upstream revision. Before
creating or significantly modifying UI, consult the upstream `STYLE.md` and
MUI patterns. Do not introduce new colors, typography scales, spacing systems,
border-radius conventions or visual patterns unless there is a specific reason
to extend the design system. Check both light and dark modes.

# Solution-pack documentation boundary

Public documentation in this repository covers installing and operating existing
solution packs only: inspection, planning, application, checks, recovery, and
sample-data safety. Keep pack-authoring tutorials, manifest/resource examples,
packaging recipes, and pack-development design notes in the private
`attricat/attricat-solution-pack-example` repository, not here or in either
language of the public docs site. Generic Core blueprint/extension documentation
and runtime/conformance-test contracts remain in Core; do not remove validation
or tests to hide the format.

# Development

The app's development server is already running through Process Compose.

## Worktree development server

Run `just setup` from the repository root before any other `just` recipe. It
creates `.env` and writes the assigned `POSTGRES_PORT`, `API_PORT`, `WEB_PORT`,
`DOCS_PORT` values to the ignored `.worktree` file. Then run `just dev` to
start the worktree-local stack. Source that file whenever a later verification
step needs the running services:

```sh
source .worktree
curl http://127.0.0.1:$API_PORT/health
open http://127.0.0.1:$WEB_PORT
open http://127.0.0.1:$DOCS_PORT
```

Do not assume the default Vite or API ports; parallel worktrees receive unique
ports. Use `just sql` to open `psql` inside the worktree's PostgreSQL container,
and `just down` to stop the worktree database.

Use SQLx for all migration operations. Create new migrations with
`sqlx migrate add --source apps/api/migrations <description>` instead of adding
migration files manually.

## Catalog inspection

To inspect catalog state, query the API through the CLI first:

```sh
cargo run -p cli -- <command>
# or, after building/installing it:
acli <command>
```

`acli` loads the worktree `.env` by default and uses its `CATALOG_API_URL`.
Supply `--token` or `CATALOG_TOKEN` when the requested endpoint needs a personal
API token. Use `just sql` only after the CLI cannot provide the required read or
when diagnosing the database implementation; do not bypass the API with SQL for
ordinary catalog inspection.

## Extension integration verification

When changing extension installation, runtime, artifact storage, event dispatch,
permissions, or client contributions, validate the host integration against the
sibling example extension when it is available at
`../../attricat-extension-example` (two levels above this worktree). Build and
package it, side-load the generated archive into this worktree's running server,
grant its requested permissions, enable it, and exercise a real extension
event/action. Unit tests alone are not sufficient for these changes. See
[Extension integration testing](docs/extensions.md#local-extension-integration-testing).

## Database policy

SQL migrations are declarative only. They may define tables, columns, indexes,
foreign keys, `NOT NULL`, `UNIQUE`, and `CHECK` constraints, plus required types
and extensions. Never add SQL functions, procedures, triggers, views, RLS
policies, or `DO` blocks. Business authorization, validation, state transitions,
audit behavior, rate limiting, and retention live in Rust repository/application
code and use explicit transactions.

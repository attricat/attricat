# Contributing

For what Attricat is and how to self-host it, see the [README](README.md).

## Run it locally

You need:

- Docker or another compatible container runtime
- Rust
- Node.js 24 and `pnpm` 11
- `just`, `process-compose`, and `watchexec`

From the repository root:

```sh
just setup   # once per checkout
just dev     # starts everything
```

`just setup` creates `.env` from `.env.example` and picks ports for this
checkout, so several worktrees can run side by side. It writes those ports and
the URLs below to `.worktree`. Run it before any other `just` command.

`just dev` starts PostgreSQL, Mailpit, Jaeger and RustFS in containers, then
the API, the file worker, the web app and the docs site. Press `Ctrl-C` to stop
it. That also stops and removes the containers.

To find the URLs:

```sh
source .worktree
echo $WEB_URL
```

| Variable | Opens |
| --- | --- |
| `WEB_URL` | The web app |
| `DOCS_URL` | The documentation site |
| `MAILPIT_UI_URL` | Email sent by the local server |
| `JAEGER_UI_URL` | Traces from the API and file worker |
| `RUSTFS_UI_URL` | The local S3-compatible file store |

To open `psql` in this checkout's database, run `just sql`. To run only the
documentation site in `apps/docs`, use `pnpm --dir apps/docs dev`.

[Getting Started](docs/index.md#getting-started) covers migrations and tests.

## Sign in

1. Open `/login` in the web app.
2. Enter the workspace identifier `default.local` and select **Continue**.
3. Sign in with `CATALOG_BOOTSTRAP_OWNER_EMAIL` and its password from `.env`.

## Guidelines

**UI.** The web app uses a pinned copy of the
[Attricat design system](apps/catalog-web/design/). Read the
[style guide](https://github.com/attricat/design/blob/main/STYLE.md) before
changing the interface, and check both light and dark mode.

**Database.** Migrations only declare structure: tables, columns, indexes,
foreign keys, `NOT NULL`, `UNIQUE` and `CHECK` constraints, and required types
or extensions. Do not add SQL functions, procedures, triggers, views, RLS
policies or `DO` blocks. Authorization, validation, state changes, rate
limits, auditing and retention belong in the Rust code, inside repository
transactions.

## Further reading

- [Database model](docs/database.md)
- [Building and side-loading extensions](docs/extensions.md#local-extension-integration-testing)
- [Demo catalog generator](examples/generate.md)

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="apps/catalog-web/design/assets/logos/wordmark-dark.svg">
    <img alt="Attricat" src="apps/catalog-web/design/assets/logos/wordmark-light.svg" width="242" height="48">
  </picture>
</p>

Attricat is a catalog for structured records whose shape changes over time.

You describe a kind of record with a **blueprint**, which lists its fields and
their types. Records built from a blueprint are **entities**. When you change a
blueprint, Attricat saves it as a new revision. Each entity keeps the exact
revision it was created with.

The project has three parts:

| Part | Path | What it is |
| --- | --- | --- |
| API | `apps/api` | Rust (Axum) server and SQLx database migrations |
| CLI | `apps/catalog-cli` | Command-line client for the API, with JSON output |
| Web app | `apps/catalog-web` | React and Vite interface |

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

To open `psql` in this checkout's database, run `just sql`.

[Getting Started](docs/index.md#getting-started) covers migrations and tests.

## Sign in

1. Open `/login` in the web app.
2. Enter the workspace identifier `default.local` and select **Continue**.
3. Sign in with `CATALOG_BOOTSTRAP_OWNER_EMAIL` and its password from `.env`.

## Conversational agents (optional)

Users can ask an LLM to read and change the catalog.

To turn it on, set `LLM_API_KEY` for the API process, and `LLM_BASE_URL` or
`LLM_MODEL` if you need them. Restart the API, then open **Conversations** in
the web app. Without a key, agents are off and everything else works as usual.

Before you turn it on:

- Conversation text and tool results go to the configured provider. The
  browser never sees the provider key.
- Read-only tools run on their own. Every change to the catalog, including
  changes from scheduled runs, waits for a person to approve it. Review the
  proposed arguments and change summary before you approve.
- An approved scheduled run writes with the permissions of the user who started
  it.
- Use a provider account with no more access than it needs.
- Give the `agents.run` permission only to people who are allowed to request
  catalog changes.

See [agent configuration](docs/configuration.md#agent-provider) and the
[agent API](docs/api.md#agents) for limits and details.

## Run the published image

To try Attricat without building it, use
[`deploy/compose.quickstart.yml`](deploy/compose.quickstart.yml). It runs
`ghcr.io/attricat/attricat:latest` with PostgreSQL, RustFS and Mailpit. It
reuses those service definitions from `apps/api/compose.yml`, so run it from a
checkout of this repository:

```sh
export ATTRICAT_OWNER_PASSWORD='pick-a-password'
docker compose -f deploy/compose.quickstart.yml up -d --wait
```

Open <http://localhost:3000/login> and sign in with workspace `default.local`,
email `owner@example.com` and the password you exported. Email the server
sends, such as password resets, shows up in Mailpit at <http://localhost:8025>.

To use different host ports, set `ATTRICAT_PORT` or `MAILPIT_UI_PORT`. To run a
specific build, set `ATTRICAT_IMAGE`, for example to
`ghcr.io/attricat/attricat:<commit-sha>`. Run
`docker compose -f deploy/compose.quickstart.yml pull` to update to the newest
image. `down` stops the stack and `down -v` also deletes its data. The image is
built for amd64 only, so Apple Silicon and other arm64 machines run it under
emulation.

This setup is for local evaluation. It binds to localhost, uses plain HTTP and
has fixed internal credentials. To deploy for real, see [Deploy](#deploy).

## Deploy

Attricat ships as one container image with three commands:

- `migrate` applies database migrations
- `api` runs the API and serves the web app
- `file-worker` generates file variants and deletes stored files

You provide PostgreSQL, private S3-compatible storage, SMTP and monitoring.
Start from [`deploy/compose.yml`](deploy/compose.yml) and read
[Production operations](docs/operations.md) for rollout, backup, restore and
rollback.

## Documentation

The documentation site lives in `apps/docs`. Run it alone with
`pnpm --dir apps/docs dev`.

- [Documentation index](docs/index.md)
- [Writing blueprints](docs/blueprints.md)
- [Relationships walkthrough](examples/relationships/README.md)
- [Demo catalog generator](examples/generate.md)
- [CLI](docs/cli.md)
- [API reference](docs/api.md)
- [Configuration](docs/configuration.md)
- [File storage and retention](docs/configuration.md#file-storage-operations)
- [Accounts](docs/authentication.md)
- [Database model](docs/database.md)
- [Building and side-loading extensions](docs/extensions.md#local-extension-integration-testing)

## Contributing

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

## License

The code is licensed under the [GNU Affero General Public License, version
3](LICENSE) (`AGPL-3.0-only`).

The design assets in `apps/catalog-web/design/` come from the separate
[Attricat design repository](https://github.com/attricat/design) and follow its
license. Third-party dependencies keep their own licenses.

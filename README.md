# Catalog

Catalog is a versioned catalog engine with a Rust API and CLI plus a React web
application. Blueprints define versioned entity schemas; entities retain the
exact revision they were created with.

## Run Locally

Install a Docker-compatible container runtime, Rust, Node.js 18 or newer,
`just`, `process-compose`, and `watchexec`. Then, from the repository root:

```sh
npm install --prefix apps/catalog-web
just dev
```

`just dev` assigns persistent, worktree-specific ports, writes them to the
ignored `.catalog-worktree` file, and creates `.env` from `.env.example`. It
prints the web URL when it starts. Stop `process-compose` with `Ctrl-C`; the
PostgreSQL container remains available until stopped with:

```sh
just down
```

See [Getting Started](docs/index.md#getting-started) for database, migration,
and test instructions.

## Documentation

- [Documentation index](docs/index.md)
- [Blueprint authoring](docs/blueprints.md)
- [Catalog CLI](docs/cli.md)
- [API reference](docs/api.md)
- [Configuration reference](docs/configuration.md)
- [Database model](docs/database.md)
- [Manual test-data generator](examples/generate.md)
- [Relationships walkthrough](examples/relationships/README.md)

## Applications

- `apps/api`: Axum API and SQLx migrations.
- `apps/catalog-cli`: JSON-first HTTP command-line client.
- `apps/catalog-web`: React and Vite web application.

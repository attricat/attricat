# Catalog Documentation

## Getting Started

1. Install a Docker-compatible runtime, Rust, Node.js 18 or newer, `just`,
   `process-compose`, and `watchexec`.
2. From the repository root, run `cp .env.example .env` and
   `npm install --prefix apps/catalog-web`.
3. Run `just dev`. It starts PostgreSQL and watches the API and web app.
4. Verify the API at `http://127.0.0.1:3000/health` and open the Vite URL
   shown in the process output, normally `http://127.0.0.1:5173`.

The API applies embedded SQLx migrations when it starts. To run them manually:

```sh
sqlx migrate run --source apps/api/migrations --database-url "$DATABASE_URL"
```

Stop the application processes with `Ctrl-C`. PostgreSQL persists until you
run `docker compose -f apps/api/compose.yml down`.

See [Configuration](configuration.md) for connection, proxy, and API limit
settings.

## Learn The Model

- [Blueprint authoring](blueprints.md): schema definitions, versions, contexts,
  views, and validation.
- [Database model](database.md): persisted model, value history, projections,
  contexts, and publication behavior.
- [JSON Schema validation](json-schema-validation.md): attribute and entity
  validation contracts.

## Use Catalog

- [Catalog CLI](cli.md): automation and command-line workflows.
- [API reference](api.md): HTTP routes and API behavior.
- [Relationships walkthrough](../examples/relationships/README.md): create
  blueprints, entities, contextual values, and relationships end to end.
- [Manual test-data generator](../examples/generate.md): create a larger,
  additive development data set.
- [Internal roadmap](roadmap.html): dated planning material, not a capability
  reference.

## Customize The Web App

- [View configuration](views.md): declarative blueprint layouts.
- [Component authoring](component-authoring.md): registered web components.
- [Frontend conventions](frontend.md): contributor conventions and tests.

## Test The Web App

The Playwright suite provisions an isolated PostgreSQL container and launches
separate API and Vite processes; it never changes the development database.

```sh
npx playwright install chromium --prefix apps/catalog-web
npm run test:e2e --prefix apps/catalog-web
```

With Colima, ensure its Docker runtime is running. The test setup resolves the
active Docker context and disables Ryuk because Colima cannot mount its socket
into the cleanup sidecar.

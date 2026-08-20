# Catalog Documentation

## Getting Started

1. Install a Docker-compatible runtime, Rust, Node.js 18 or newer, `just`,
   `process-compose`, and `watchexec`.
2. From the repository root, run `npm install --prefix apps/catalog-web`.
3. Run `just dev`. It creates `.env` from `.env.example`, assigns persistent
   ports for this worktree, starts PostgreSQL, and watches the API and web app.
4. Open the worktree-specific Vite URL printed by `just dev`; the selected
   ports are also recorded in the ignored `.catalog-worktree` file.

The API applies embedded SQLx migrations when it starts. To run them manually:

```sh
sqlx migrate run --source apps/api/migrations --database-url "$DATABASE_URL"
```

Stop the application processes with `Ctrl-C`. PostgreSQL persists until you
run `just down`.

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
- [Relationship tree facets](search-facets.md): filter explorer results through
  a contextual hierarchy with roll-up counts.
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
separate API and Vite processes on fresh ports; it never changes the development
database.

```sh
npx playwright install chromium --prefix apps/catalog-web
npm run test:e2e --prefix apps/catalog-web
```

With Colima, ensure its Docker runtime is running. The test setup resolves the
active Docker context and disables Ryuk because Colima cannot mount its socket
into the cleanup sidecar.

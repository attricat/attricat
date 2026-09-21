# Catalog Documentation

## Getting Started

1. Install a Docker-compatible runtime, Rust, Node.js 22.14.0, pnpm 11.25.0 (Corepack manages this automatically), `just`,
   `process-compose`, and `watchexec`.
2. Run `just setup` once. It installs frontend dependencies, creates `.env`
   from `.env.example`, and assigns persistent ports for this worktree.
3. Run `just dev`. It starts PostgreSQL, Mailpit, Jaeger, and RustFS, then
   watches the API, file worker, and web app.
4. Source `.catalog-worktree`, then open the worktree-specific `WEB_URL` and
   `JAEGER_UI_URL`. Jaeger shows local API and file-worker traces.

The API applies embedded SQLx migrations when it starts. To run them manually:

```sh
just migrate
```

Stop `just dev` with `Ctrl-C`. Its exit trap also runs `just down`, stopping
and removing the local containers.

See [Configuration](configuration.md) for connection, proxy, file-storage, and API limit
settings. The local RustFS-backed production-compatibility check is opt-in and
uses the same `S3_*` settings as the API:

```sh
just dev # leave this running in another terminal to start RustFS
just test-s3-compat
```

Run `just setup` first. `test-s3-compat` uses that generated environment and
runs the ignored RustFS compatibility test.

## Learn The Model

- [Backend crate architecture](backend-architecture.md): compilation boundaries,
  dependency direction, ownership, and compatibility policy.
- [Blueprint authoring](blueprints.md): schema definitions, versions, contexts,
  views, and validation.
- [Database model](database.md): persisted model, value history, projections,
  contexts, and publication behavior.
- [Tags, labels, and classifications](classifications.md): model controlled
  vocabularies with entities, relationships, contexts, and hierarchies.
- [Domain eventing](eventing.md): transactional outbox contract and delivery semantics.
- [Extensions](extensions.md): strict manifest, permissions, webhook, and lifecycle contracts.
- [Authentication and identity adapters](authentication.md): local password
  lifecycle plus the provider-neutral external identity seam.
- [JSON Schema validation](json-schema-validation.md): attribute and entity
  validation contracts.

## Use Catalog

- [Catalog CLI](cli.md): automation and command-line workflows.
- [API reference](api.md): HTTP routes and API behavior.
- [Relationship tree facets](search-facets.md): filter explorer results through
  a contextual hierarchy with roll-up counts.
- [Relationship-aware Explore search](relationship-aware-search.md): planned
  graph traversal and structured query-language semantics.
- [Relationships walkthrough](../examples/relationships/README.md): create
  blueprints, entities, contextual values, and relationships end to end.
- [Demo catalog generator](../examples/generate.md): create deterministic,
  industry-scoped demo and local performance data sets.
- [Internal roadmap](roadmap.html): dated planning material, not a capability
  reference.

## Customize The Web App

- [View configuration](views.md): declarative blueprint layouts.
- [Component authoring](component-authoring.md): registered web components.
- [Web-component event bridge](web-component-events.md): validated host and component DOM events.
- [Frontend conventions](frontend.md): contributor conventions and tests.

## Test The Web App

The Playwright suite provisions an isolated PostgreSQL container and launches
separate API and Vite processes on fresh ports; it never changes the development
database.

```sh
pnpm --dir apps/catalog-web exec playwright install chromium
pnpm --dir apps/catalog-web test:e2e
```

With Colima, ensure its Docker runtime is running. The test setup resolves the
active Docker context and disables Ryuk because Colima cannot mount its socket
into the cleanup sidecar.

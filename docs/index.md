# Catalog Documentation

## Getting Started

1. Install a Docker-compatible runtime, Rust, Node.js 24 (the CI version),
   `pnpm` 11, `just`, `process-compose`, and `watchexec`.
2. Run `just setup` once. It installs frontend dependencies, creates `.env`
   from `.env.example`, and assigns persistent ports for this worktree.
3. Run `just dev`. It starts PostgreSQL, Mailpit, Jaeger, and RustFS, then
   watches the API, file worker, and web app.
4. Source `.worktree`, then open the worktree-specific `WEB_URL` and
   `JAEGER_UI_URL`. Jaeger shows local API and file-worker traces.

The API applies embedded SQLx migrations when it starts. To run them manually:

```sh
just migrate
```

Stop `just dev` with `Ctrl-C`. Its exit trap also runs `just down`, stopping
and removing the local containers.

See [Configuration](configuration.md) for connection, proxy, file-storage, and
API limits. See [Production operations](operations.md) for rollout, recovery,
and backup/restore procedures. To run the optional production-compatibility test
against local RustFS, use the same `S3_*` settings as the API:

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
- [Status attributes](status-control.md): single-select display/editing, transition
  constraints, contextual inheritance, and stale-edit protection.
- [Database model](database.md): persisted model, value history, projections,
  contexts, and publication behavior.
- [Tags, labels, and classifications](classifications.md): model controlled
  vocabularies with entities, relationships, contexts, and hierarchies.
- [Domain eventing](eventing.md): transactional outbox and delivery semantics.
- [Workflows](workflows.md): versioned triggers and bounded entity actions.
- [Rules](rules.md): blueprint checks and finding lifecycles.
- [Extensions](extensions.md): manifest, permissions, runtime, and lifecycle contracts;
  webhook delivery is not implemented.
- [Authentication and identity adapters](authentication.md): local password
  management and the interface for external identity providers.
- [Field-level read restrictions](field-level-read-restrictions.md): audit of
  every read surface (attributes cannot be hidden today) and the design for
  restricted attributes.
- [JSON Schema validation](json-schema-validation.md): attribute and entity
  validation contracts.

## Use Catalog

- [Install and operate solution packs](solution-packs.md): inspect existing archives,
  review plans, apply, verify, and recover. See [optional sample data](solution-pack-sample-data.md)
  for opt-in, automation warnings, retry, and cleanup limits.
- [Catalog CLI](cli.md): automation and command-line workflows.
- [API reference](api.md): HTTP routes and API behavior.
- [Saved searches](saved-views.md): named views, share links, and access rules.
- [Relationship tree facets](search-facets.md): filter explorer results through
  a contextual hierarchy with roll-up counts.
- [Relationship-aware Explore search](relationship-aware-search.md): explicit
  graph traversal and structured query-language semantics.
- [Relationships walkthrough](../examples/relationships/README.md): create
  blueprints, entities, contextual values, and relationships end to end.
- [Demo catalog generator](../examples/generate.md): create deterministic,
  industry-scoped demo and local performance data sets.

## Customize The Web App

- [View configuration](views.md): declarative blueprint layouts.
- [Component authoring](component-authoring.md): registered web components.
- [Web-component event bridge](web-component-events.md): validated host and component DOM events.
- [Frontend conventions](frontend.md): contributor conventions and tests.
- [Performance validation](performance-validation.md): compile and Explorer workload measurements.

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

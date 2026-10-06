# Backend crate architecture

The API is a composition application built from focused workspace crates. The split keeps changes to HTTP handlers, domain contracts, solution-pack planning, agent execution, persistence, object storage, workers, and the Wasmtime host in separate Cargo compilation units.

## Dependency direction

```text
apps/api → catalog-http → catalog-agent-runtime → catalog-workers
                    │                 │                   │
                    └→ catalog-extension-runtime ─────────┘
catalog-workers → catalog-repository → catalog-solution-pack
                       │                         │
                       ├→ catalog-storage         └→ catalog-extension-manifest
                       ├→ catalog-domain
                       ├→ catalog-events
                       └→ catalog-cache ←── catalog-extension-runtime
```

Arrows point to dependencies and show key edges, not every edge; both runtime crates also depend directly on the repository and storage, and `apps/api` builds the configured cache.

`catalog-solution-pack` depends on extension manifest contracts, never the reverse. `catalog-storage` is an independent object-store port and S3 adapter. `catalog-cache` is a leaf: it knows nothing about the catalog, and the repository decides what is cached under which key (see [Caching](caching.md)). Both the agent runtime and the extension runtime depend on the generic task-handler contract in `catalog-workers`; workers depend on neither. This keeps the graph acyclic while allowing each runtime crate to provide its own task-handler adapter. HTTP depends on the agent runtime for provider-backed endpoints and configuration.

No library crate may depend on `api`. `apps/api` alone owns process startup, database migration embedding, concrete dependency construction, and the `api` and `file-worker` binary names.

## Ownership

| Crate | Responsibility | Heavy dependency isolated here |
| --- | --- | --- |
| `catalog-domain` | Domain DTOs, account value objects, agent/task contracts, and product safety limits | — |
| `catalog-events` | Versioned event envelopes, names, and validation | SQLx row decoding only |
| `catalog-extension-manifest` | Extension manifests, archive validation, and containment policy | archive codecs |
| `catalog-solution-pack` | Solution-pack contracts, archive/sample-data validation, and pure planning | image/archive codecs |
| `catalog-repository` | SQLx repository, transactional application services, registry and installer coordination | SQLx/Reqwest |
| `catalog-storage` | Object-store interface, S3 adapter, storage configuration | AWS SDK |
| `catalog-cache` | Query cache (process-memory L1, optional Redis L2), shared rate limits, cache configuration | Moka/fred |
| `catalog-workers` | Task/event supervision and workflow/rule/file execution | image/EXIF clients |
| `catalog-agent-runtime` | Agent provider, tools, execution, and task handler | provider clients |
| `catalog-extension-runtime` | Wasmtime component host and WIT bindings | Wasmtime |
| `catalog-http` | Axum routes, authentication, transport limits, mail delivery, and request telemetry | Axum/OTLP/Lettre |
| `api` | Startup, migrations, dependency injection, compatibility re-exports | composition only |

The facade intentionally preserves historical paths such as `api::model`, `api::repository`, `api::solution_packs`, `api::agent_worker`, and `api::http`. New library code should import the owning crate directly. These re-exports can be removed only as a separately reviewed compatibility change.

## Repository scopes and maintenance

`CatalogRepository` carries a mandatory workspace scope. `SystemRepository`
(`CatalogRepository<SystemScope>`) exposes authentication, explicit-scope identity
operations, bootstrap, and process-level queue/maintenance operations, but cannot
call workspace data methods. Derive a workspace repository from the authenticated
workspace or claimed task before accessing catalog data. There is no fallback to
the bootstrap workspace.

`for_workspace` shares the bounded pool and performs no SQL. Provision default
contexts explicitly with `initialize_workspace`; startup repairs existing active
workspaces once, rather than attempting writes during every request. Transaction-owning
helpers must reuse the caller's connection instead of acquiring another pool slot.

Workers poll execution and lease renewal concurrently through
`catalog-workers`' `heartbeat::with_heartbeat`. Do not await database renewal
inside a selected timer branch: that stops polling the job, which may hold the
lock or pool connection renewal needs. Lease loss drops the execution future
before cleanup runs. This does not replace transactional task fencing at each
mutation's commit boundary.

History retention runs periodically in bounded transactions. File uploads commit
durable object-key intents before S3 writes and consume them in the same
transaction as file persistence. Unfinished intents become cleanup work after the
request deadline plus one hour. Cleanup claims fence late finalization, retry
failed S3 deletions, and survive cancellation and process restarts. Do not delete
objects on an ambiguous database-commit error.

## Authorization mutation locking

Role and membership mutations take the workspace's `FOR NO KEY UPDATE` lock
before locking roles, memberships or credentials. Invitation acceptance,
onboarding and browser-session renewal use the same outer lock. In particular,
renewal must acquire it before the old session row; otherwise a concurrent
revocation can miss the replacement inserted after its SQL statement snapshot.
Recheck management permission, ownership and
delegated permissions on the transaction's connection after acquiring the locks;
preflight checks alone cannot authorize a mutation after a concurrent role edit
or ownership transfer. Keep this ordering when adding authority-changing paths.
The workspace lock serializes these mutations without blocking unrelated
foreign-key checks.

## Stable assets

- SQL migrations remain under `apps/api/migrations`; `api::MIGRATOR` embeds that directory and SQLx CLI commands keep the same source path.
- The extension host WIT contract (`catalog:host`) is owned by `crates/extension-runtime/wit-host`; released snapshots in `crates/extension-runtime/wit-released` are frozen.
- HTTP routes, JSON representations, event contracts, CLI behavior, configuration names, and deployed binary names are compatibility boundaries.
- Database migrations remain declarative. Authorization, validation, transitions, audit, retention, and retries remain explicit Rust code and transactions.

## Compile-time measurement

Run the isolated benchmark without disturbing ordinary build artifacts:

```sh
scripts/benchmark-api-compile.sh all
```

The script sets a dedicated `CARGO_TARGET_DIR`, records a clean check, a warm check, then representative domain, repository, HTTP, and runtime edit/recheck timings. Use `cold` or `warm` to limit a run. Compare results on the same machine and toolchain; absolute clean-build time includes third-party dependencies and is less useful than incremental edit/recheck time.

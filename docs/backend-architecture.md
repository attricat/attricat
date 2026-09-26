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
                       └→ catalog-events
```

Arrows point to dependencies and show key edges, not every edge; both runtime crates also depend directly on the repository and storage.

`catalog-solution-pack` depends on extension manifest contracts, never the reverse. `catalog-storage` is an independent object-store port and S3 adapter. Both the agent runtime and the extension runtime depend on the generic task-handler contract in `catalog-workers`; workers depend on neither. This keeps the graph acyclic while allowing each runtime crate to provide its own task-handler adapter. HTTP depends on the agent runtime for provider-backed endpoints and configuration.

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
| `catalog-workers` | Task/event supervision and workflow/rule/file execution | image/EXIF clients |
| `catalog-agent-runtime` | Agent provider, tools, execution, and task handler | provider clients |
| `catalog-extension-runtime` | Wasmtime component host and WIT bindings | Wasmtime |
| `catalog-http` | Axum routes, authentication, transport limits, mail delivery, and request telemetry | Axum/OTLP/Lettre |
| `api` | Startup, migrations, dependency injection, compatibility re-exports | composition only |

The facade intentionally preserves historical paths such as `api::model`, `api::repository`, `api::solution_packs`, `api::agent_worker`, and `api::http`. New library code should import the owning crate directly. These re-exports can be removed only as a separately reviewed compatibility change.

## Stable assets

- SQL migrations remain under `apps/api/migrations`; `api::MIGRATOR` embeds that directory and SQLx CLI commands keep the same source path.
- Extension WIT 1.0 and 1.1 contracts are owned by `crates/extension-runtime/wit` and `wit-next`. Moving the files did not change package, world, or interface definitions.
- HTTP routes, JSON representations, event contracts, CLI behavior, configuration names, and deployed binary names are compatibility boundaries.
- Database migrations remain declarative. Authorization, validation, transitions, audit, retention, and retries remain explicit Rust code and transactions.

## Compile-time measurement

Run the isolated benchmark without disturbing ordinary build artifacts:

```sh
scripts/benchmark-api-compile.sh all
```

The script sets a dedicated `CARGO_TARGET_DIR`, records a clean check, a warm check, then representative domain, repository, HTTP, and runtime edit/recheck timings. Use `cold` or `warm` to limit a run. Compare results on the same machine and toolchain; absolute clean-build time includes third-party dependencies and is less useful than incremental edit/recheck time.

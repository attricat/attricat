# Demo Catalog Generator

`generate.mjs` creates a deterministic, fictional **ByteForge Components** PC-parts catalog through the public HTTP API. It is intended for demos, API verification, and local Explorer performance testing. It never connects to PostgreSQL.

## Prerequisites

Start the local stack and create a personal access token in the profile section. The token needs:

- `blueprints.read`, `blueprints.write`, and `blueprints.publish`;
- `contexts.read` and `contexts.write`;
- `entities.read`, `entities.write`, and `entities.publish`.

File upload, metadata, and download authorization uses the existing entity permissions; there is intentionally no separate `files.read` personal-token permission.

Pass the token as `CATALOG_TOKEN`. The generator refuses non-local targets unless `ALLOW_NON_LOCAL_GENERATOR_TARGET=1` is explicitly set.

```sh
just setup
just dev
CATALOG_TOKEN=cat_pat_... just generate
```

## Profiles

Profiles preserve the same component types, distributions, and bounded compatibility relationships. They differ only in scale.

| Profile  | Total entities | Use                                           |
| -------- | -------------: | --------------------------------------------- |
| `micro`  |          1,022 | Fast API-backed verification and resume tests |
| `small`  |         10,022 | Normal local demo (the default)               |
| `medium` |        100,022 | Routine local performance dataset             |
| `large`  |      1,000,022 | Full local benchmark dataset                  |

```sh
CATALOG_TOKEN=cat_pat_... just generate micro
CATALOG_TOKEN=cat_pat_... just generate medium
CATALOG_TOKEN=cat_pat_... just generate large
```

The current industry pack is `pc-components`. It creates fictional manufacturers, hierarchical categories, product families, sellable SKUs, and reference-data entities for product types, interface standards, and form factors. Families relate to those controlled classifications as well as category and manufacturer; sellable SKUs relate to their family and bounded compatibility links, reaching category, manufacturer, and technical classifications through that family. Each generated product image is assigned as a SKU's `main_photo`; a fixed, small set of documentation files is assigned to `product_files`. This does not grow with the selected profile.

## Long-running runs

The generator writes an atomic local checkpoint under `.catalog-generator/`, keyed by industry, schema version, profile, and seed. It prints entity and request progress, rolling throughput, retries, and an ETA. A large run can take a substantial time because every entity and relationship is written through the API.

```sh
# Preview the exact work plan without contacting the API.
node examples/generate.mjs --size large --dry-run

# Continue a stopped run with the matching checkpoint.
CATALOG_TOKEN=cat_pat_... just generate-resume large

# Inspect checkpoint state without making API requests.
node examples/generate.mjs --size large --status

# Machine-readable progress for log collection.
CATALOG_TOKEN=cat_pat_... node examples/generate.mjs --size medium --progress json
```

Use `--no-files` for a pure Explorer dataset when file processing is not required. The generator publishes each completed entity to every enabled publication channel by default; pass `--no-publish` to leave generated entities unpublished. If no channels are enabled, publication is a successful no-op. The generator marks the checkpoint `benchmark_ready` only after all planned writes, file processing (when enabled), and API-level sample verification complete.

## Safety and reproducibility

Use a dedicated local database for `large` runs. Do not point the generator at shared or production environments. Choose a different `--seed` to create a separate deterministic dataset; an existing matching checkpoint requires `--resume` rather than silently adding a second copy.

Each record is tagged and annotated with its generator dataset identity. Checkpoints retain the exact seed, profile, schema version, phase, and counts so an interrupted run reports whether it is resumable or complete.

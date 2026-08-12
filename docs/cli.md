# Catalog CLI

`catalog` is a JSON-first HTTP client for the Catalog API. It is intended for
automation, agents, and shell scripts; successful API response JSON is written
unchanged to stdout and structured errors are written to stderr.

## Start Locally

```sh
docker compose -f apps/api/compose.yml up -d
DATABASE_URL=postgres://postgres:postgres@localhost:5432/catalog cargo run -p api
```

The client targets `http://127.0.0.1:3000` by default. Override it with:

```sh
catalog --server http://127.0.0.1:3000 health
CATALOG_SERVER=http://127.0.0.1:3000 catalog health
```

During development, run the workspace binary without installing it:

```sh
cargo run -p catalog-cli -- health
```

## Commands

```sh
catalog health

catalog blueprint create --file product.toml
catalog blueprint create --stdin
catalog blueprint revision <blueprint-id> --file product-v2.toml
catalog blueprint get <blueprint-id>
catalog blueprint get-version <blueprint-id> <version>
catalog blueprint resolve <code>
catalog blueprint resolve <code> --version <version>

catalog context create --file locale.toml
catalog context create --code en-GB --data '{"language":"en-GB"}'
catalog context get en-GB

catalog entity create --file product.toml
catalog entity get <entity-id>
catalog entity list --blueprint <code> --related-from <entity-id> --relationship <attribute-code> [--limit <limit>] [--cursor <cursor>]
catalog entity preview <entity-id> [--relationship-depth <depth>] [--relationship-limit <limit>]

catalog value append <entity-id> --file values.toml
catalog value current <entity-id>
catalog value replace <entity-id> --file relationships.toml
catalog value remove <entity-id> --file relationships.toml
```

`entity preview` resolves active relationship targets inline by default to one
level. Set `--relationship-depth 0` for scalar values only, or request deeper
traversal up to the API's configured maximum. `--relationship-limit` bounds
inline targets per relationship; use `entity list` for paginated browsing.

Blueprint files are sent unchanged as the API's TOML `definition`, preserving
the raw-source hash. See `database.md` for the blueprint grammar.

## Input Files

Entity file:

```toml
blueprint_id = "00000000-0000-0000-0000-000000000001"
blueprint_version = 1

[projections]
source = "fixture"
```

Context file:

```toml
code = "en-GB"

[data]
language = "en-GB"
```

Value file:

```toml
[[values]]
kind = "scalar"
attribute_code = "title"
value = "Blue shirt"

[[values]]
kind = "relationship"
attribute_code = "related_products"
target_entity_id = "00000000-0000-0000-0000-000000000004"
```

Scalar payloads must match the blueprint attribute type. TOML maps naturally to
the API's typed JSON values: quoted text for `string`, decimals for `number`,
integers for `integer`, booleans for `boolean`, TOML dates and RFC 3339
timestamps for `date` and `datetime`. A `time` value requires a wall-clock time
and IANA timezone:

```toml
[[values]]
kind = "scalar"
attribute_code = "order_cutoff"
value = { time = "09:30:00", time_zone = "America/New_York" }
```

Each value must provide exactly one of `attribute_code` or `attribute_id`. Codes
are resolved against the source entity's pinned blueprint version.

Relationship replacement and removal files use this shape. `replace` makes the
listed targets the complete current set for each attribute; `remove` unlinks only
listed currently linked targets.

```toml
[[relationships]]
attribute_code = "categories"
target_entity_ids = ["00000000-0000-0000-0000-000000000004"]
```

## Errors

The client writes errors such as this to stderr:

```json
{
  "error": {
    "code": "invalid_blueprint_definition",
    "message": "blueprints must define at least one attribute",
    "status": 422
  }
}
```

Exit codes are `0` for success, `2` for local input/configuration errors, `3`
for transport errors, `4` for API error responses, and `5` for invalid API JSON.

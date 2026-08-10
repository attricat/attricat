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
catalog entity get-by-code <blueprint-id> <code>
catalog entity preview <entity-id>

catalog value append <entity-id> --file values.toml
catalog value current <entity-id>
```

Blueprint files are sent unchanged as the API's TOML `definition`, preserving
the raw-source hash. See `database.md` for the blueprint grammar.

## Input Files

Entity file:

```toml
code = "shirt-001"
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
attribute_id = "00000000-0000-0000-0000-000000000002"
value = "Blue shirt"

[[values]]
kind = "relationship"
attribute_id = "00000000-0000-0000-0000-000000000003"
target_entity_id = "00000000-0000-0000-0000-000000000004"
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

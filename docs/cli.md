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

catalog blueprint list [--include-drafts]
catalog blueprint create --file product.toml
catalog blueprint create --stdin
catalog blueprint revision <blueprint-id> --file product-v2.toml
catalog blueprint publish <blueprint-id> <version>
catalog blueprint get <blueprint-id>
catalog blueprint get-version <blueprint-id> <version>
catalog blueprint resolve <code> [--include-drafts]
catalog blueprint resolve <code> --version <version>

catalog context list
catalog context create --file locale.toml
catalog context create --code en_GB --data '{"language":"en-GB"}' --parent-id <context-id>
catalog context get en-GB
catalog context update <context-id> --parent-id <context-id> --data '{"language":"en-GB"}'
catalog context delete <context-id>

catalog entity create --blueprint product --values values.toml [--version <version>] [--context-id <context-id>]
catalog entity get <entity-id>
catalog entity delete <entity-id>
catalog entity list --blueprint <code> --related-from <entity-id> --relationship <attribute-code> [--limit <limit>] [--cursor <cursor>]
catalog entity preview <entity-id> [--relationship-depth <depth>] [--relationship-limit <limit>]
catalog entity resolved-preview <entity-id> --context-id <context-id>
catalog entity search --blueprint <code> [--version <version>] [--query <text>] [--size <size>] [--cursor <cursor>]
catalog entity form <entity-id>
catalog entity update <entity-id> [--values values.toml] [--relationships relationships.toml] [--remove-values removals.toml] [--context-id <context-id>]
catalog entity migrate <entity-id>
catalog entity migrate-bulk --blueprint <code> --from-version <version> [--size <size>] [--dry-run]

catalog value append <entity-id> --file values.toml --context-id <context-id>
catalog value current <entity-id>
catalog value replace <entity-id> --file relationships.toml --context-id <context-id>
catalog value remove <entity-id> --file relationships.toml --context-id <context-id>

catalog workspace member list
catalog workspace member set-state <member-id> --state active|inactive
catalog workspace member grant <member-id> --role-id <role-id> --scope-type <scope> --scope-target-id <id>
catalog workspace member revoke-grant <member-id> <grant-id>
catalog workspace member transfer-ownership <member-id>
catalog workspace role list
catalog workspace role permission list
catalog workspace role assignable-role list
catalog workspace role create --code <code> --permissions <json-or-file>
catalog workspace role update <role-id> --code <code> --permissions <json-or-file>
catalog workspace role duplicate <role-id> [--code <code>]
catalog workspace role retire <role-id> [--replacement-role-id <role-id>]
catalog workspace invitation list
catalog workspace invitation create --email <email> --role-id <role-id> --scope-type <scope> --scope-target-id <id> --expires-at <rfc3339>
catalog workspace invitation revoke <invitation-id>
catalog workspace invitation accept --secret-stdin
catalog workspace user create --email <email> [--display-name <name>] [--invite-role-id <role-id> --scope-type <scope> --scope-target-id <id> --expires-at <rfc3339>]
catalog workspace user set-password --onboarding-secret-stdin --invitation-secret-stdin --password-stdin

catalog token list
catalog token create --label <label> --permissions <json-or-file> [--expires-at <rfc3339>]
catalog token revoke <token-id>
```

## Workspace administration

Pass a personal API token with `--token` or `CATALOG_TOKEN`. Workspace commands
always operate on the workspace selected by that bearer credential; they never
accept a workspace ID or tenant header. `--permissions` accepts either a JSON
array (for example, `'["entities.read"]'`) or a path to a JSON file. Personal
API-token secrets are emitted only in their successful JSON response. Workspace
invitation and onboarding links are delivered by the configured mail adapter and
are never emitted by the CLI. Use `catalog workspace invitation accept --secret-stdin`
only when consuming the secret from the delivered link without placing it in shell
history or argv:

```sh
printf '%s' "$CATALOG_INVITATION_SECRET" | catalog --token "$CATALOG_TOKEN" workspace invitation accept --secret-stdin
# Password setup reads onboarding secret, invitation secret, then password from separate stdin lines.
printf '%s\n%s\n%s\n' "$ONBOARDING_SECRET" "$INVITATION_SECRET" "$PASSWORD" | catalog workspace user set-password --onboarding-secret-stdin --invitation-secret-stdin --password-stdin
```

`entity preview` calls `/entities/{id}/preview` and resolves active relationship
targets inline by default to one level. Its response contains `entity` metadata
and a context-keyed preview under `context`. Set `--relationship-depth 0` for
scalar values only, or request deeper traversal up to the API's configured
maximum. `--relationship-limit` bounds inline targets per relationship; use
`entity list` for paginated browsing.

`entity resolved-preview` resolves attributes for one requested context, including
enriched relationship sets, and includes the context that supplied each value.
`entity form` returns the pinned
blueprint, current direct facts, and form context. `entity update` uses the v1
atomic form endpoint: scalar values append history, relationship files replace
the supplied relationship sets, and removal files remove scalar overrides.
`entity delete` soft-deletes the entity. Its value and relationship history are
retained, while normal reads and relationship previews no longer expose it.

## Blueprint Migrations

`entity migrate` previews an entity against the latest published revision of its
blueprint and migrates it when all current values are compatible. The API copies
current values to the target revision, validates them, rebuilds projections, and
records the migration atomically.

`entity migrate-bulk` pages entities pinned to `--from-version` and runs that
same operation for each entity. It never chooses a target revision: every
entity targets the latest published revision. Use `--dry-run` to classify entities
without migrating them. The command returns a JSON summary with `ready`,
`needs_input`, `blocked`, and `failed` entries. It skips `needs_input` and
`blocked` entities. Resolve missing, incompatible, or conditionally required
values through the entity upgrade page before rerunning the bulk command.

For example, after creating `seed_product` v2:

```sh
catalog entity migrate-bulk --blueprint seed_product --from-version 1 --dry-run
catalog entity migrate-bulk --blueprint seed_product --from-version 1
```

Blueprint files are sent unchanged as the API's TOML `definition`, preserving
the raw-source hash. See [Blueprint Authoring](blueprints.md) for the grammar.

Blueprint creation and revision commands create drafts. Publish a revision
explicitly before using it for entities or migrations:

```sh
catalog blueprint publish <blueprint-id> <version>
```

Default blueprint listing and resolution return only published revisions. Use
`catalog blueprint list --include-drafts` to inspect drafts; exact
`blueprint get-version` reads are also draft-visible.

## Input Files

Context file:

```toml
code = "en-GB"
parent_id = "00000000-0000-4000-8000-000000000001"

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

Every value and relationship needs a context. Supply `--context-id` to use it
for all entries in a file, or set `context_id` on an individual TOML entry. Use
`catalog context list` or `catalog context get default` to obtain IDs. The
persisted `default` root is `00000000-0000-4000-8000-000000000001`.

Relationship replacement and removal files use this shape. `replace` makes the
listed targets the complete current set for each attribute; `remove` unlinks only
listed currently linked targets.

```toml
[[relationships]]
attribute_code = "categories"
target_entity_ids = ["00000000-0000-0000-0000-000000000004"]
```

Scalar removals used by `entity update --remove-values` use:

```toml
[[remove_values]]
attribute_code = "subtitle"
```

## Manual Test Data

The Node.js generator creates at least 100 parent products and two variants for
each parent, alongside categories, colors, relationships, contexts, and
blueprints that exercise the supported blueprint features. Start the API, then
run:

```sh
just generate
```

See [`examples/generate.md`](../examples/generate.md) for configuration and a
complete description of the generated data.

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

# Related Categories And Colors

This walkthrough creates the three blueprints, a category, a color, and a
product with contextual scalar overrides and constrained relationships. The
product includes every native scalar type: string, number, integer, boolean,
date, datetime, and a wall-clock time with an IANA timezone.

Run `just setup` once before other `just` recipes, then start `just dev` in
another terminal (unless the development stack is already running). From the
repository root, use an authenticated CLI session or set `CATALOG_TOKEN` to a
personal token with blueprint, context, and entity write permissions. These
examples use `jq` to capture API IDs; see [CLI authentication](../../docs/cli.md#browser-authentication)
for session setup. Run the following commands from the repository root:

```sh
CATEGORY_BLUEPRINT_ID=$(cargo run -p cli -- blueprint create --file examples/relationships/category.toml | jq -r '.blueprint.id')
COLOR_BLUEPRINT_ID=$(cargo run -p cli -- blueprint create --file examples/relationships/color.toml | jq -r '.blueprint.id')
PRODUCT_BLUEPRINT_ID=$(cargo run -p cli -- blueprint create --file examples/relationships/product.toml | jq -r '.blueprint.id')

cargo run -p cli -- blueprint publish "$CATEGORY_BLUEPRINT_ID" 1
cargo run -p cli -- blueprint publish "$COLOR_BLUEPRINT_ID" 1
cargo run -p cli -- blueprint publish "$PRODUCT_BLUEPRINT_ID" 1
```

Create the entities in the default context and retain their IDs:

```sh
CATEGORY_ID=$(cargo run -p cli -- entity create --blueprint category --values examples/relationships/category-values.toml | jq -r '.entity.id')
COLOR_ID=$(cargo run -p cli -- entity create --blueprint color --values examples/relationships/color-values.toml | jq -r '.entity.id')
PRODUCT_ID=$(cargo run -p cli -- entity create --blueprint product --values examples/relationships/product-values.toml | jq -r '.entity.id')
```

Create two child contexts and append the provided localized overrides:

```sh
PL_CONTEXT_ID=$(cargo run -p cli -- context create --code PL --data '{"language":"pl"}' | jq -r '.id')
PL_B2C_CONTEXT_ID=$(cargo run -p cli -- context create --code PL-b2c --data '{"channel":"b2c"}' --parent-id "$PL_CONTEXT_ID" | jq -r '.id')

cargo run -p cli -- value append "$PRODUCT_ID" --file examples/relationships/product-pl-values.toml --context-id "$PL_CONTEXT_ID"
cargo run -p cli -- value append "$PRODUCT_ID" --file examples/relationships/product-pl-b2c-values.toml --context-id "$PL_B2C_CONTEXT_ID"
```

Create `relationships.toml` with the captured target entity IDs:

```sh
cat > relationships.toml <<EOF
[[relationships]]
attribute_code = "categories"
target_entity_ids = ["$CATEGORY_ID"]

[[relationships]]
attribute_code = "colors"
target_entity_ids = ["$COLOR_ID"]
EOF
```

Replace the complete relationship sets in the default context, then read the
direct and context-resolved previews:

```sh
cargo run -p cli -- value replace "$PRODUCT_ID" --file relationships.toml --context-id 00000000-0000-4000-8000-000000000001
cargo run -p cli -- entity preview "$PRODUCT_ID" --relationship-depth 1
cargo run -p cli -- entity resolved-preview "$PRODUCT_ID" --context-id "$PL_B2C_CONTEXT_ID"
```

`target_blueprint` in `product.toml` ensures categories cannot point to colors,
and vice versa.

# Related Categories And Colors

This walkthrough creates the three blueprints, a category, a color, and a
product with contextual scalar overrides and constrained relationships. The
product includes every native scalar type: string, number, integer, boolean,
date, datetime, and a wall-clock time with an IANA timezone.

Start the services in one terminal, then run the remaining commands from a
second terminal at the repository root. The examples use `jq` to capture API
IDs.

```sh
just dev
```

From the second terminal:

```sh
CATEGORY_BLUEPRINT_ID=$(cargo run -p catalog-cli -- blueprint create --file examples/relationships/category.toml | jq -r '.blueprint.id')
COLOR_BLUEPRINT_ID=$(cargo run -p catalog-cli -- blueprint create --file examples/relationships/color.toml | jq -r '.blueprint.id')
PRODUCT_BLUEPRINT_ID=$(cargo run -p catalog-cli -- blueprint create --file examples/relationships/product.toml | jq -r '.blueprint.id')

cargo run -p catalog-cli -- blueprint publish "$CATEGORY_BLUEPRINT_ID" 1
cargo run -p catalog-cli -- blueprint publish "$COLOR_BLUEPRINT_ID" 1
cargo run -p catalog-cli -- blueprint publish "$PRODUCT_BLUEPRINT_ID" 1
```

Create the entities in the default context and retain their IDs:

```sh
CATEGORY_ID=$(cargo run -p catalog-cli -- entity create --blueprint category --values examples/relationships/category-values.toml | jq -r '.entity.id')
COLOR_ID=$(cargo run -p catalog-cli -- entity create --blueprint color --values examples/relationships/color-values.toml | jq -r '.entity.id')
PRODUCT_ID=$(cargo run -p catalog-cli -- entity create --blueprint product --values examples/relationships/product-values.toml | jq -r '.entity.id')
```

Create two child contexts and append the provided localized overrides:

```sh
PL_CONTEXT_ID=$(cargo run -p catalog-cli -- context create --code PL --data '{"language":"pl"}' | jq -r '.id')
PL_B2C_CONTEXT_ID=$(cargo run -p catalog-cli -- context create --code PL-b2c --data '{"channel":"b2c"}' --parent-id "$PL_CONTEXT_ID" | jq -r '.id')

cargo run -p catalog-cli -- value append "$PRODUCT_ID" --file examples/relationships/product-pl-values.toml --context-id "$PL_CONTEXT_ID"
cargo run -p catalog-cli -- value append "$PRODUCT_ID" --file examples/relationships/product-pl-b2c-values.toml --context-id "$PL_B2C_CONTEXT_ID"
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
cargo run -p catalog-cli -- value replace "$PRODUCT_ID" --file relationships.toml --context-id 00000000-0000-4000-8000-000000000001
cargo run -p catalog-cli -- entity preview "$PRODUCT_ID" --relationship-depth 1
cargo run -p catalog-cli -- entity resolved-preview "$PRODUCT_ID" --context-id "$PL_B2C_CONTEXT_ID"
```

`target_blueprint` in `product.toml` ensures categories cannot point to colors,
and vice versa.

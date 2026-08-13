#!/usr/bin/env bash
set -euo pipefail

# Seed the complete relationship example through the public catalog CLI.
catalog_bin="${CATALOG_BIN:-./target/debug/catalog}"
run_catalog() { $catalog_bin "$@"; }

default_id="$(run_catalog context get default | jq -r '.id')"

run_catalog blueprint create --file examples/relationships/category.toml >/dev/null
run_catalog blueprint create --file examples/relationships/color.toml >/dev/null
run_catalog blueprint create --file examples/relationships/product.toml >/dev/null

pl_id="$(run_catalog context create --code PL --data '{"market":"PL"}' --parent-id "$default_id" | jq -r '.id')"
pl_b2c_id="$(run_catalog context create --code PL-b2c --data '{"audience":"b2c"}' --parent-id "$pl_id" | jq -r '.id')"
pl_b2c_web_id="$(run_catalog context create --code PL-b2c-web --data '{"channel":"web"}' --parent-id "$pl_b2c_id" | jq -r '.id')"

relationship_file="$(mktemp)"
trap 'rm -f "$relationship_file"' EXIT

run_catalog entity create --blueprint category --values examples/relationships/category-values.toml --context-id "$default_id" >/dev/null
run_catalog entity create --blueprint color --values examples/relationships/color-values.toml --context-id "$default_id" >/dev/null
category_entity="$(run_catalog entity search --blueprint category --query Shirts | jq -r '.items[0].id')"
color_entity="$(run_catalog entity search --blueprint color --query Navy | jq -r '.items[0].id')"
test "$category_entity" != "null"
test "$color_entity" != "null"
product_entity="$(run_catalog entity create --blueprint product --values examples/relationships/product-values.toml --context-id "$default_id" | jq -r '.id')"
run_catalog value append "$product_entity" --file examples/relationships/product-pl-values.toml --context-id "$pl_id" >/dev/null
run_catalog value append "$product_entity" --file examples/relationships/product-pl-b2c-values.toml --context-id "$pl_b2c_id" >/dev/null

printf '[[relationships]]\nattribute_code = "categories"\ntarget_entity_ids = ["%s"]\n\n[[relationships]]\nattribute_code = "colors"\ntarget_entity_ids = ["%s"]\n' "$category_entity" "$color_entity" > "$relationship_file"
run_catalog value replace "$product_entity" --file "$relationship_file" --context-id "$default_id" >/dev/null

printf 'Seeded product %s with contexts default -> PL -> PL-b2c -> PL-b2c-web\n' "$product_entity"

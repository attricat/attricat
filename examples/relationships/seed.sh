#!/usr/bin/env bash
set -euo pipefail

# Seed the complete relationship example through the public catalog CLI.
catalog_bin="${CATALOG_BIN:-./target/debug/catalog}"
run_catalog() { $catalog_bin "$@"; }

default_id="$(run_catalog context get default | jq -r '.id')"

category_blueprint="$(run_catalog blueprint create --file examples/relationships/category.toml | jq -r '.blueprint.id')"
color_blueprint="$(run_catalog blueprint create --file examples/relationships/color.toml | jq -r '.blueprint.id')"
product_blueprint="$(run_catalog blueprint create --file examples/relationships/product.toml | jq -r '.blueprint.id')"

pl_id="$(run_catalog context create --code PL --data '{"market":"PL"}' --parent-id "$default_id" | jq -r '.id')"
pl_b2c_id="$(run_catalog context create --code PL-b2c --data '{"audience":"b2c"}' --parent-id "$pl_id" | jq -r '.id')"
pl_b2c_web_id="$(run_catalog context create --code PL-b2c-web --data '{"channel":"web"}' --parent-id "$pl_b2c_id" | jq -r '.id')"

entity_file="$(mktemp)"
relationship_file="$(mktemp)"
trap 'rm -f "$entity_file" "$relationship_file"' EXIT

create_entity() {
  local blueprint_id="$1"
  printf 'blueprint_id = "%s"\nblueprint_version = 1\n' "$blueprint_id" > "$entity_file"
  run_catalog entity create --file "$entity_file" | jq -r '.id'
}

category_entity="$(create_entity "$category_blueprint")"
run_catalog value append "$category_entity" --file examples/relationships/category-values.toml --context-id "$default_id" >/dev/null
color_entity="$(create_entity "$color_blueprint")"
run_catalog value append "$color_entity" --file examples/relationships/color-values.toml --context-id "$default_id" >/dev/null
product_entity="$(create_entity "$product_blueprint")"
run_catalog value append "$product_entity" --file examples/relationships/product-values.toml --context-id "$default_id" >/dev/null
run_catalog value append "$product_entity" --file examples/relationships/product-pl-values.toml --context-id "$pl_id" >/dev/null
run_catalog value append "$product_entity" --file examples/relationships/product-pl-b2c-values.toml --context-id "$pl_b2c_id" >/dev/null

printf '[[relationships]]\nattribute_code = "categories"\ntarget_entity_ids = ["%s"]\n\n[[relationships]]\nattribute_code = "colors"\ntarget_entity_ids = ["%s"]\n' "$category_entity" "$color_entity" > "$relationship_file"
run_catalog value replace "$product_entity" --file "$relationship_file" --context-id "$default_id" >/dev/null

printf 'Seeded product %s with contexts default -> PL -> PL-b2c -> PL-b2c-web\n' "$product_entity"

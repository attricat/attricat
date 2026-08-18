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

create_product() {
  local title="$1"
  local price="$2"
  local stock="$3"
  local values_file
  values_file="$(mktemp)"
  printf '[[values]]\nkind = "scalar"\nattribute_code = "title"\nvalue = "%s"\n\n[[values]]\nkind = "scalar"\nattribute_code = "price"\nvalue = %s\n\n[[values]]\nkind = "scalar"\nattribute_code = "stock_on_hand"\nvalue = %s\n\n[[values]]\nkind = "scalar"\nattribute_code = "available"\nvalue = true\n' "$title" "$price" "$stock" > "$values_file"
  run_catalog entity create --blueprint product --values "$values_file" --context-id "$default_id" | jq -r '.id'
  rm -f "$values_file"
}

replace_product_relationships() {
  local entity_id="$1"
  shift
  {
    printf '[[relationships]]\nattribute_code = "categories"\ntarget_entity_ids = ["%s"]\n\n' "$category_entity"
    printf '[[relationships]]\nattribute_code = "colors"\ntarget_entity_ids = ["%s"]\n' "$color_entity"
    if [ "$#" -gt 0 ]; then
      printf '\n[[relationships]]\nattribute_code = "variants"\ntarget_entity_ids = ['
      local variant_id
      local separator=""
      for variant_id in "$@"; do
        printf '%s"%s"' "$separator" "$variant_id"
        separator=", "
      done
      printf ']\n'
    fi
  } > "$relationship_file"
  run_catalog value replace "$entity_id" --file "$relationship_file" --context-id "$default_id" >/dev/null
}

run_catalog entity create --blueprint category --values examples/relationships/category-values.toml --context-id "$default_id" >/dev/null
run_catalog entity create --blueprint color --values examples/relationships/color-values.toml --context-id "$default_id" >/dev/null
category_entity="$(run_catalog entity search --blueprint category --query Shirts | jq -r '.items[0].id')"
color_entity="$(run_catalog entity search --blueprint color --query Navy | jq -r '.items[0].id')"
test "$category_entity" != "null"
test "$color_entity" != "null"
product_entity="$(run_catalog entity create --blueprint product --values examples/relationships/product-values.toml --context-id "$default_id" | jq -r '.id')"
run_catalog value append "$product_entity" --file examples/relationships/product-pl-values.toml --context-id "$pl_id" >/dev/null
run_catalog value append "$product_entity" --file examples/relationships/product-pl-b2c-values.toml --context-id "$pl_b2c_id" >/dev/null

navy_small="$(create_product "Navy shirt / Small" 49.95 8)"
navy_large="$(create_product "Navy shirt / Large" 49.95 11)"
jacket_entity="$(create_product "Trail jacket" 129.00 16)"
jacket_small="$(create_product "Trail jacket / Small" 129.00 5)"
jacket_large="$(create_product "Trail jacket / Large" 129.00 7)"

replace_product_relationships "$product_entity" "$navy_small" "$navy_large"
replace_product_relationships "$navy_small"
replace_product_relationships "$navy_large"
replace_product_relationships "$jacket_entity" "$jacket_small" "$jacket_large"
replace_product_relationships "$jacket_small"
replace_product_relationships "$jacket_large"

printf 'Seeded 6 products, including Navy shirt and Trail jacket variants, with contexts default -> PL -> PL-b2c -> PL-b2c-web\n'

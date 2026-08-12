# Related Categories And Colors

Create the three blueprints, then create a category, color, and product entity
using their returned blueprint IDs. Append the supplied scalar value files. The
product example includes every native scalar type: string, number, integer,
boolean, date, datetime, and a wall-clock time with an IANA timezone.

Use a relationship file with the created target entity IDs:

```toml
[[relationships]]
attribute_code = "categories"
target_entity_ids = ["<category-entity-id>"]

[[relationships]]
attribute_code = "colors"
target_entity_ids = ["<color-entity-id>"]
```

Assign the complete set and read it back:

```sh
catalog value replace <product-entity-id> --file relationships.toml
catalog entity preview <product-entity-id> --relationship-depth 1
```

`target_blueprint` in `product.toml` ensures categories cannot point to colors,
and vice versa.

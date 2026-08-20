# Manual Test Data Generator

`generate.mjs` seeds additive, realistic-looking catalog data through the public
HTTP API. It uses Node.js 18 or later and has no package dependencies.

Start the development services, then run it from the repository root:

```sh
node examples/generate.mjs
```

Set `CATALOG_SERVER` to target another API URL. `PRODUCT_COUNT` controls the
number of parent products and is clamped to a minimum of 100:

```sh
CATALOG_SERVER=http://127.0.0.1:3000 PRODUCT_COUNT=150 node examples/generate.mjs
```

Set `SEED_BLUEPRINTS_ONLY=1` to create updated blueprint revisions without
creating entities or contexts:

```sh
SEED_BLUEPRINTS_ONLY=1 node examples/generate.mjs
```

The generator loads its blueprint definitions from `examples/generator/products/` and
creates or reuses the `product_seo`, `category`, `color`, and `product`
blueprints. The product blueprint demonstrates
mixins and version-pinned includes, selected attributes, tags, scalar and entity
JSON Schema validation, display configuration, context policies, all native
scalar types, constrained relationships, and configured views and components.
When a generated blueprint definition changes, the generator creates a new blueprint
revision. Existing entities remain pinned to their original revision and can be
upgraded through the entity migration flow.
Generated revisions are published automatically after their pinned mixin revisions,
so generated entities always use published blueprints.

Each run creates six categories, six colors, at least 100 parent products, and
two variants for every parent. Products and variants receive category and color
relationships; parents additionally receive their variant relationships. A
`seed-us -> seed-us-web` context chain and a product with overrides demonstrate
context fallback. Runs are additive, so use a fresh development database when a
clean data set is needed.

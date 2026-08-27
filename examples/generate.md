# Manual Test Data Generator

`generate.mjs` seeds additive, realistic-looking catalog data through the public
HTTP API. It uses Node.js 18 or later and has no package dependencies.

Start the development services, then run it from the repository root:

The generator authenticates every request with a required personal access token.
Create one in the profile section of the app with these permissions:
`blueprints.read`, `blueprints.write`, `blueprints.publish`, `contexts.read`,
`contexts.write`, and `entities.write`. Then provide it as `CATALOG_TOKEN`.
`just generate` loads this variable from `.env`.

```sh
CATALOG_TOKEN=cat_pat_... just generate
```

Set `CATALOG_SERVER` to target another API URL. `PRODUCT_COUNT` controls the
number of parent products and is clamped to a minimum of 100:

```sh
CATALOG_SERVER=http://127.0.0.1:3000 PRODUCT_COUNT=150 node examples/generate.mjs
```

Set `SEED_BLUEPRINTS_ONLY=1` to create updated blueprint revisions without
creating entities or contexts:

```sh
SEED_BLUEPRINTS_ONLY=1 just generate
```

The generator loads its blueprint definitions from `examples/generator/products/` and
creates or reuses the `product_seo`, `category`, `color`, and `product`
blueprints. The product blueprint demonstrates
mixins and version-pinned includes, selected attributes, tags, scalar and entity
JSON Schema validation, dropdown-option and screen views, context policies, all native
scalar types, constrained relationships, and configured views and components.
Category, color, and product detail views also demonstrate lazy,
cursor-paginated incoming relationship lists.
When a generated blueprint definition changes, the generator creates a new blueprint
revision. Existing entities remain pinned to their original revision and can be
upgraded through the entity migration flow.
Generated revisions are published automatically after their pinned mixin revisions,
so generated entities always use published blueprints.

Each run creates a fictional but realistic Alder & Row fashion assortment: seven
clothing, footwear, outerwear, activewear, and accessories categories; six named
colors; and at least 100 parent products. Product titles, descriptions, prices,
SKUs, launch codes, stock, SEO metadata, and size variants are assembled from a
curated deterministic collection, so the same product number is easy to locate
and reason about during manual tests. Products and variants receive category and
color relationships; parents additionally receive their variant relationships.
Accessories have one `One Size` variant, while apparel and footwear receive their
applicable real-world size range. A `seed-us -> seed-us-web` context chain and a
Studio Leather Tote with regional overrides demonstrate context fallback. Runs
are additive, so use a fresh development database when a clean data set is
needed.

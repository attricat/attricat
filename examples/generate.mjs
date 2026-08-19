#!/usr/bin/env node

const server = (process.env.CATALOG_SERVER ?? 'http://127.0.0.1:3000').replace(/\/$/, '');
const productCount = Math.max(Number.parseInt(process.env.PRODUCT_COUNT ?? '100', 10) || 100, 100);
const defaultContextId = '00000000-0000-4000-8000-000000000001';
const blueprintsOnly = process.env.SEED_BLUEPRINTS_ONLY === '1';

const seoBlueprint = `
format_version = 1
code = "seed_product_seo"
name = "Seed Product SEO"
kind = "mixin"

[[attributes]]
code = "meta_title"
value_type = "string"
tags = ["seo"]

[[attributes]]
code = "meta_description"
value_type = "string"
context_fallback = "none"
`;

const categoryBlueprint = `
format_version = 1
code = "seed_category"
name = "Seed Category"
kind = "entity"

[display.dropdown_option]
fields = ["name", "slug"]
separator = " / "

[views.detail]
type = "stack"
children = [{ type = "heading", text = "Category" }, { type = "field", field = "name" }]

[views.edit]
type = "stack"
children = [{ type = "field", field = "name" }, { type = "field", field = "slug" }]

[views.table]
type = "table"
fields = ["name", "slug"]

[[attributes]]
code = "name"
value_type = "string"
tags = ["searchable"]

[[attributes]]
code = "slug"
value_type = "string"
`;

const colorBlueprint = `
format_version = 1
code = "seed_color"
name = "Seed Color"
kind = "entity"

[display.dropdown_option]
fields = ["name", "hex"]
separator = " / "

[views.detail]
type = "stack"
children = [{ type = "heading", text = "Color" }, { type = "field", field = "name" }]

[views.edit]
type = "stack"
children = [{ type = "field", field = "name" }, { type = "field", field = "hex" }]

[views.table]
type = "table"
fields = ["name", "hex"]

[[attributes]]
code = "name"
value_type = "string"

[[attributes]]
code = "hex"
value_type = "string"
`;

const productBlueprint = `
format_version = 1
code = "seed_product"
name = "Seed Product"
kind = "entity"
entity_schema = '{"type":"object","required":["title","sku","price","launch_code"]}'

[[includes]]
alias = "seo"
code = "seed_product_seo"
version = 1

[display.dropdown_option]
fields = ["title", "sku"]
separator = " / "

[views.detail]
type = "stack"
children = [
  { type = "stack", component = { id = "catalog.entity_heading", version = 1 }, children = [{ type = "field", field = "title" }, { type = "text", text = "Generated catalog product" }] },
  { type = "section", children = [{ type = "grid", children = [{ type = "field", field = "price", component = { id = "catalog.field_display", version = 1 } }, { type = "field", field = "stock_on_hand" }] }] },
  { type = "tabs", tabs = [{ label = "Availability", children = [{ type = "accordion", sections = [{ label = "Schedule", children = [{ type = "field", field = "available_on" }, { type = "field", field = "released_at" }, { type = "field", field = "order_cutoff" }] }] }] }] },
  { type = "divider" },
  { type = "relationship_list", field = "categories", component = { id = "catalog.relationship_list_display", version = 1 } },
  { type = "relationship_list", field = "colors" },
  { type = "relationship_list", field = "variants" },
]

[views.edit]
type = "stack"
children = [
  { type = "grid", children = [{ type = "field", field = "title", component = { id = "catalog.field_edit", version = 1 } }, { type = "field", field = "sku" }, { type = "field", field = "launch_code" }] },
  { type = "grid", children = [{ type = "field", field = "price" }, { type = "field", field = "stock_on_hand" }] },
  { type = "field", field = "description" },
  { type = "accordion", sections = [{ label = "Availability", children = [{ type = "field", field = "available" }, { type = "field", field = "available_on" }, { type = "field", field = "released_at" }, { type = "field", field = "order_cutoff" }] }] },
  { type = "section", children = [{ type = "field", field = "meta_title" }] },
  { type = "relationship_list", field = "categories", component = { id = "catalog.relationship_list_edit", version = 1 } },
  { type = "relationship_list", field = "colors" },
  { type = "relationship_list", field = "variants" },
]

[views.table]
type = "table"
fields = ["title", "sku", "price", "stock_on_hand", "available"]
component = { id = "catalog.table_display", version = 1 }

[[attributes]]
code = "title"
value_type = "string"
tags = ["display", "searchable"]

[[attributes]]
code = "sku"
value_type = "string"
context_editable = "default"

[[attributes]]
code = "description"
value_type = "string"
context_fallback = "none"

[[attributes]]
code = "price"
value_type = "number"
value_schema = '{"type":"number","minimum":100}'

[[attributes]]
code = "stock_on_hand"
value_type = "string"
context_editable = "default"

[[attributes]]
code = "available"
value_type = "boolean"

[[attributes]]
code = "launch_code"
value_type = "string"

[[attributes]]
code = "available_on"
value_type = "date"

[[attributes]]
code = "released_at"
value_type = "datetime"

[[attributes]]
code = "order_cutoff"
value_type = "time"

[[attributes]]
code = "meta_title"
from = "seo.meta_title"

[[attributes]]
code = "categories"
value_type = "relationship"
target_blueprint = "seed_color"

[[attributes]]
code = "colors"
value_type = "relationship"
target_blueprint = "seed_color"

[[attributes]]
code = "variants"
value_type = "relationship"
target_blueprint = "seed_product"
context_fallback = "none"
`;

const request = async (path, options = {}) => {
  const response = await fetch(`${server}${path}`, {
    ...options,
    headers: { 'content-type': 'application/json', ...options.headers },
  });
  const body = await response.json().catch(() => null);
  if (!response.ok) {
    const message = body?.error?.message ?? response.statusText;
    throw new Error(`${options.method ?? 'GET'} ${path} failed (${response.status}): ${message}`);
  }
  return body;
};

const ensureBlueprint = async (code, definition) => {
  const publish = (blueprint) => request(
    `/blueprints/${blueprint.blueprint.id}/versions/${blueprint.blueprint.version}/publish`,
    { method: 'POST' },
  );
  try {
    const current = await request(`/blueprints/by-code/${code}?include_drafts=true`);
    if (current.blueprint.definition === definition) {
      return current.blueprint.status === 'published' ? current : publish(current);
    }
    const draft = await request(`/blueprints/${current.blueprint.id}/versions`, {
      method: 'POST',
      body: JSON.stringify({ definition }),
    });
    return publish(draft);
  } catch (error) {
    if (!error.message.includes('(404)')) throw error;
    const draft = await request('/blueprints', { method: 'POST', body: JSON.stringify({ definition }) });
    return publish(draft);
  }
};

const ensureContext = async (code, data, parentId) => {
  try {
    return await request(`/contexts/${code}`);
  } catch (error) {
    if (!error.message.includes('(404)')) throw error;
    return request('/contexts', { method: 'POST', body: JSON.stringify({ code, data, parent_id: parentId }) });
  }
};

const scalar = (attribute_code, value, context_id = defaultContextId) => ({
  kind: 'scalar', attribute_code, context_id, value,
});

const createEntity = async (blueprint, values) => request('/v1/entities', {
  method: 'POST',
  body: JSON.stringify({ blueprint: { code: blueprint }, values }),
});

const replaceRelationships = (entityId, relationships) => request(`/entities/${entityId}/relationships/replace`, {
  method: 'POST',
  body: JSON.stringify({ relationships }),
});

const productValues = (name, sku, index) => {
  const price = Number((19.99 + (index % 40) * 3.75).toFixed(2));
  return [
    scalar('title', name),
    scalar('sku', sku),
    scalar('description', `A generated ${name.toLowerCase()} for manual catalog testing.`),
    scalar('price', price),
    scalar('stock_on_hand', String(5 + (index % 60))),
    scalar('available', index % 7 !== 0),
    scalar('launch_code', `LAUNCH-${String(index).padStart(4, '0')}`),
    scalar('available_on', `2026-${String((index % 12) + 1).padStart(2, '0')}-${String((index % 27) + 1).padStart(2, '0')}`),
    scalar('released_at', `2026-${String((index % 12) + 1).padStart(2, '0')}-15T10:30:00Z`),
    scalar('order_cutoff', { time: '16:30:00', time_zone: 'America/New_York' }),
    scalar('meta_title', `${name} | Seed Store`),
  ];
};

const run = async () => {
  await request('/health');
  await ensureBlueprint('seed_product_seo', seoBlueprint);
  await Promise.all([
    ensureBlueprint('seed_category', categoryBlueprint),
    ensureBlueprint('seed_color', colorBlueprint),
  ]);
  await ensureBlueprint('seed_product', productBlueprint);
  if (blueprintsOnly) {
    console.log('Ensured seed blueprint revisions.');
    return;
  }

  const regional = await ensureContext('seed-us', { market: 'US' }, defaultContextId);
  const web = await ensureContext('seed-us-web', { channel: 'web' }, regional.id);

  const categoryNames = ['Apparel', 'Footwear', 'Accessories', 'Outdoor', 'Home', 'Technology'];
  const colorNames = [['Black', '#111111'], ['Navy', '#172554'], ['Red', '#dc2626'], ['Green', '#15803d'], ['Sand', '#d6c7a1'], ['White', '#f8fafc']];
  const categoryIds = await Promise.all(categoryNames.map(async (name) => {
    const entity = await createEntity('seed_category', [scalar('name', name), scalar('slug', name.toLowerCase())]);
    return entity.id;
  }));
  const colorIds = await Promise.all(colorNames.map(async ([name, hex]) => {
    const entity = await createEntity('seed_color', [scalar('name', name), scalar('hex', hex)]);
    return entity.id;
  }));

  for (let index = 1; index <= productCount; index += 1) {
    const name = `Seed Product ${String(index).padStart(3, '0')}`;
    const parent = await createEntity('seed_product', productValues(name, `SEED-${String(index).padStart(4, '0')}`, index));
    const variants = await Promise.all(['Small', 'Large'].map((size, variantIndex) => createEntity(
      'seed_product',
      productValues(`${name} / ${size}`, `SEED-${String(index).padStart(4, '0')}-${size[0]}`, index * 10 + variantIndex),
    )));
    const categoryId = categoryIds[index % categoryIds.length];
    const colorId = colorIds[index % colorIds.length];
    await Promise.all([
      replaceRelationships(parent.id, [
        { attribute_code: 'categories', context_id: defaultContextId, target_entity_ids: [categoryId] },
        { attribute_code: 'colors', context_id: defaultContextId, target_entity_ids: [colorId] },
        { attribute_code: 'variants', context_id: defaultContextId, target_entity_ids: variants.map(({ id }) => id) },
      ]),
      ...variants.map((variant) => replaceRelationships(variant.id, [
        { attribute_code: 'categories', context_id: defaultContextId, target_entity_ids: [categoryId] },
        { attribute_code: 'colors', context_id: defaultContextId, target_entity_ids: [colorId] },
      ])),
    ]);

    if (index % 10 === 0) console.log(`Created ${index}/${productCount} parent products`);
  }

  const sample = await createEntity('seed_product', productValues('Seed Context Sample', 'SEED-CONTEXT', 999));
  await request(`/entities/${sample.id}/values`, {
    method: 'POST',
    body: JSON.stringify({ values: [
      scalar('title', 'Seed Context Sample (US)', regional.id),
      scalar('price', 89.99, regional.id),
      scalar('title', 'Seed Context Sample (US Web)', web.id),
    ] }),
  });

  console.log(`Created ${productCount} parent products, ${productCount * 2} variants, ${categoryIds.length} categories, and ${colorIds.length} colors.`);
};

run().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});

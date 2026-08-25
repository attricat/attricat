#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const server = (process.env.CATALOG_SERVER ?? 'http://127.0.0.1:3000').replace(/\/$/, '');
const productCount = Math.max(Number.parseInt(process.env.PRODUCT_COUNT ?? '100', 10) || 100, 100);
const defaultContextId = '00000000-0000-4000-8000-000000000001';
const blueprintsOnly = process.env.SEED_BLUEPRINTS_ONLY === '1';

const CATALOG_TOKEN = `${process.env.CATALOG_TOKEN}`.trim();

const definition = (file) => readFile(new URL(`./generator/products/${file}`, import.meta.url), 'utf8');

const request = async (path, options = {}) => {
  const response = await fetch(`${server}${path}`, {
    ...options,
    headers: { 'content-type': 'application/json', 'authorization': `Bearer ${CATALOG_TOKEN }`, ...options.headers },
  });
  const body = await response.json().catch(() => null);
  if (!response.ok) {
    const message = body?.error?.message ?? response.statusText;
    throw new Error(`${options.method ?? 'GET'} ${path} failed (${response.status}): ${message}`);
  }
  return body;
};

const ensureBlueprint = async (code, source) => {
  const publish = (blueprint) => request(
    `/blueprints/${blueprint.blueprint.id}/versions/${blueprint.blueprint.version}/publish`,
    { method: 'POST' },
  );
  try {
    const current = await request(`/blueprints/by-code/${code}?include_drafts=true`);
    if (current.blueprint.definition === source) {
      return current.blueprint.status === 'published' ? current : publish(current);
    }
    return publish(await request(`/blueprints/${current.blueprint.id}/versions`, {
      method: 'POST', body: JSON.stringify({ definition: source }),
    }));
  } catch (error) {
    if (!error.message.includes('(404)')) throw error;
    return publish(await request('/blueprints', {
      method: 'POST', body: JSON.stringify({ definition: source }),
    }));
  }
};

const ensureContext = async (code, data, parentId) => {
  try {
    return await request(`/contexts/${code}`);
  } catch (error) {
    if (!error.message.includes('(404)')) throw error;
    return request('/contexts', {
      method: 'POST', body: JSON.stringify({ code, data, parent_id: parentId }),
    });
  }
};

const scalar = (attribute_code, value, context_id = defaultContextId) => ({
  kind: 'scalar', attribute_code, context_id, value,
});

const createEntity = (blueprint, values) => request('/v1/entities', {
  method: 'POST', body: JSON.stringify({ blueprint: { code: blueprint }, values }),
});

const replaceRelationships = (entityId, relationships) => request(
  `/entities/${entityId}/relationships/replace`,
  { method: 'POST', body: JSON.stringify({ relationships }) },
);

const productValues = (name, sku, index) => {
  const price = Number((109.99 + (index % 40) * 3.75).toFixed(2));
  return [
    scalar('title', name), scalar('sku', sku),
    scalar('description', `A generated ${name.toLowerCase()} for manual catalog testing.`),
    scalar('price', price), scalar('stock_on_hand', String(5 + (index % 60))),
    scalar('available', index % 7 !== 0),
    scalar('launch_code', `LAUNCH-${String(index).padStart(4, '0')}`),
    scalar('available_on', `2026-${String((index % 12) + 1).padStart(2, '0')}-${String((index % 27) + 1).padStart(2, '0')}`),
    scalar('released_at', `2026-${String((index % 12) + 1).padStart(2, '0')}-15T10:30:00Z`),
    scalar('order_cutoff', { time: '16:30:00', time_zone: 'America/New_York' }),
    scalar('meta_title', `${name} | Catalog`),
  ];
};

const run = async () => {
  await request('/health');
  const [seo, category, color, product] = await Promise.all([
    definition('product-seo.toml'), definition('category.toml'),
    definition('color.toml'), definition('product.toml'),
  ]);
  await ensureBlueprint('product_seo', seo);
  await Promise.all([ensureBlueprint('category', category), ensureBlueprint('color', color)]);
  await ensureBlueprint('product', product);
  if (blueprintsOnly) return console.log('Ensured generated blueprint revisions.');

  const regional = await ensureContext('seed-us', { market: 'US' }, defaultContextId);
  const web = await ensureContext('seed-us-web', { channel: 'web' }, regional.id);
  const categories = await Promise.all([
    ['Apparel', null], ['Footwear', 'Apparel'], ['Accessories', 'Apparel'],
    ['Outdoor', null], ['Home', null], ['Technology', null],
  ].map(async ([name, parent]) => ({
    id: (await createEntity('category', [scalar('name', name), scalar('slug', name.toLowerCase())])).id,
    name,
    parent,
  })));
  const categoryIds = categories.map(({ id }) => id);
  await Promise.all(categories.flatMap(({ id, parent }) => {
    const parentId = categories.find((category) => category.name === parent)?.id;
    return parentId ? [replaceRelationships(id, [
      { attribute_code: 'parent_category', context_id: defaultContextId, target_entity_ids: [parentId] },
    ])] : [];
  }));
  const colorIds = await Promise.all([['Black', '#111111'], ['Navy', '#172554'], ['Red', '#dc2626'], ['Green', '#15803d'], ['Sand', '#d6c7a1'], ['White', '#f8fafc']]
    .map(async ([name, hex]) => (await createEntity('color', [scalar('name', name), scalar('hex', hex)])).id));

  for (let index = 1; index <= productCount; index += 1) {
    const name = `Product ${String(index).padStart(3, '0')}`;
    const parent = await createEntity('product', productValues(name, `SKU-${String(index).padStart(4, '0')}`, index));
    const variants = await Promise.all(['Small', 'Large'].map((size, variantIndex) => createEntity(
      'product', productValues(`${name} / ${size}`, `SKU-${String(index).padStart(4, '0')}-${size[0]}`, index * 10 + variantIndex),
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

  const sample = await createEntity('product', productValues('Context Sample', 'SKU-CONTEXT', 999));
  await request(`/entities/${sample.id}/values`, { method: 'POST', body: JSON.stringify({ values: [
    scalar('title', 'Context Sample (US)', regional.id), scalar('price', 189.99, regional.id),
    scalar('title', 'Context Sample (US Web)', web.id),
  ] }) });
  console.log(`Created ${productCount} parent products, ${productCount * 2} variants, ${categoryIds.length} categories, and ${colorIds.length} colors.`);
};

run().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});

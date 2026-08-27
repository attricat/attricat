#!/usr/bin/env node

import { readFile } from 'node:fs/promises';

const server = (process.env.CATALOG_SERVER ?? 'http://127.0.0.1:3000').replace(/\/$/, '');
const productCount = Math.max(Number.parseInt(process.env.PRODUCT_COUNT ?? '100', 10) || 100, 100);
const defaultContextId = '00000000-0000-4000-8000-000000000001';
const blueprintsOnly = process.env.SEED_BLUEPRINTS_ONLY === '1';

const catalogToken = `${process.env.CATALOG_TOKEN ?? ''}`.trim();
const storeName = 'Alder & Row';

const definition = (file) => readFile(new URL(`./generator/products/${file}`, import.meta.url), 'utf8');

const request = async (path, options = {}) => {
  const headers = {
    'content-type': 'application/json',
    authorization: `Bearer ${catalogToken}`,
    ...options.headers,
  };
  const response = await fetch(`${server}${path}`, {
    ...options,
    headers,
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

const catalogColors = [
  { name: 'Black', hex: '#1c1917', code: 'BLK' },
  { name: 'Ivory', hex: '#f5f1e8', code: 'IVR' },
  { name: 'Navy', hex: '#1e3a5f', code: 'NVY' },
  { name: 'Olive', hex: '#59634a', code: 'OLV' },
  { name: 'Camel', hex: '#b68a5a', code: 'CAM' },
  { name: 'Burgundy', hex: '#6f1d2b', code: 'BRG' },
];

const catalogProducts = [
  ['WDR-LIN', "Women's Clothing", 'Linen Wrap Midi Dress', 'Breathable washed linen with a softly defined waist and side pockets.', 148, ['XS', 'S', 'M', 'L', 'XL']],
  ['WTOP-RIB', "Women's Clothing", 'Ribbed Cotton Tank', 'A close-fitting organic cotton layer with a square neckline.', 42, ['XS', 'S', 'M', 'L', 'XL']],
  ['WJN-STR', "Women's Clothing", 'High-Rise Straight Jean', 'Rigid denim with a high waist and an easy straight leg.', 128, ['24', '26', '28', '30', '32']],
  ['WKN-MER', "Women's Clothing", 'Merino Crewneck Sweater', 'Fine-gauge merino wool knitted for lightweight everyday warmth.', 118, ['XS', 'S', 'M', 'L', 'XL']],
  ['WBL-WOL', "Women's Clothing", 'Tailored Wool Blazer', 'A single-button blazer cut from Italian wool with a relaxed shoulder.', 298, ['2', '4', '6', '8', '10']],
  ['WSK-SAT', "Women's Clothing", 'Satin Bias Midi Skirt', 'Fluid satin with a bias cut that moves easily from day to evening.', 98, ['XS', 'S', 'M', 'L', 'XL']],
  ['WSH-POP', "Women's Clothing", 'Cotton Poplin Shirt', 'Crisp cotton poplin with a longer hem designed for layering.', 88, ['XS', 'S', 'M', 'L', 'XL']],
  ['WJK-QUI', 'Outerwear', 'Quilted Field Jacket', 'Lightly insulated recycled fill and a corduroy-trimmed collar.', 218, ['XS', 'S', 'M', 'L', 'XL']],
  ['MSH-OXF', "Men's Clothing", 'Oxford Button-Down Shirt', 'Midweight Oxford cloth with a button-down collar and box pleat.', 89, ['S', 'M', 'L', 'XL', 'XXL']],
  ['MPT-CHI', "Men's Clothing", 'Slim Taper Chino', 'Stretch cotton twill in a clean, tapered everyday fit.', 98, ['28', '30', '32', '34', '36']],
  ['MKN-HZF', "Men's Clothing", 'Merino Half-Zip Sweater', 'Soft merino knit with a polished metal half-zip.', 138, ['S', 'M', 'L', 'XL', 'XXL']],
  ['MOV-WOL', "Men's Clothing", 'Wool Overshirt', 'Brushed wool blend with utility pockets and horn-style buttons.', 188, ['S', 'M', 'L', 'XL', 'XXL']],
  ['MTS-JER', "Men's Clothing", 'Heavyweight Jersey Tee', 'Substantial combed cotton with a relaxed, straight fit.', 48, ['S', 'M', 'L', 'XL', 'XXL']],
  ['MJK-MAC', 'Outerwear', 'Water-Resistant Mac Coat', 'A rain-ready cotton-blend mac with a removable throat latch.', 268, ['S', 'M', 'L', 'XL', 'XXL']],
  ['WSN-CRT', 'Footwear', 'Retro Court Sneaker', 'Leather low-top sneaker with a cushioned cupsole.', 125, ['6', '7', '8', '9', '10']],
  ['WFT-BAL', 'Footwear', 'Leather Ballet Flat', 'Soft leather flat with a rounded toe and flexible sole.', 118, ['5', '6', '7', '8', '9']],
  ['MSH-LFT', 'Footwear', 'Suede Penny Loafer', 'Unlined suede loafer finished with a traditional penny slot.', 178, ['7', '8', '9', '10', '11']],
  ['MBT-CHL', 'Footwear', 'Leather Chelsea Boot', 'Goodyear-welted leather boot with elastic side panels.', 248, ['7', '8', '9', '10', '11']],
  ['BAG-STU', 'Bags & Accessories', 'Studio Leather Tote', 'Structured full-grain leather tote with a padded laptop sleeve.', 325, ['One Size']],
  ['BAG-WEK', 'Bags & Accessories', 'Weekender Canvas Duffel', 'Waxed canvas carryall with a detachable shoulder strap.', 195, ['One Size']],
  ['ACC-SIL', 'Bags & Accessories', 'Silk Twill Scarf', 'Printed silk twill scarf with hand-rolled edges.', 78, ['One Size']],
  ['ACC-LTH', 'Bags & Accessories', 'Reversible Leather Belt', 'Vegetable-tanned leather belt with black and tan sides.', 85, ['S', 'M', 'L', 'XL']],
  ['ACT-LEG', 'Activewear', 'Performance Pocket Legging', 'Four-way stretch fabric with a secure side pocket.', 92, ['XS', 'S', 'M', 'L', 'XL']],
  ['ACT-HOD', 'Activewear', 'French Terry Zip Hoodie', 'Midweight French terry with a two-way zip and clean finish.', 112, ['S', 'M', 'L', 'XL', 'XXL']],
  ['ACT-SHR', 'Activewear', 'Technical Run Short', 'Quick-drying shell short with a breathable liner.', 68, ['S', 'M', 'L', 'XL', 'XXL']],
];

const productForIndex = (index) => {
  const [styleCode, category, styleName, description, price, sizes] = catalogProducts[(index - 1) % catalogProducts.length];
  const color = catalogColors[(index - 1) % catalogColors.length];
  const sequence = String(index).padStart(3, '0');
  return {
    category,
    color,
    description,
    name: `${styleName} in ${color.name}`,
    price,
    sizes,
    sku: `AR-${styleCode}-${color.code}-${sequence}`,
    launchCode: `SS26-${styleCode}-${sequence}`,
  };
};

const productValues = (product, sku, index) => {
  const month = String(((index - 1) % 6) + 1).padStart(2, '0');
  const day = String(((index - 1) % 24) + 1).padStart(2, '0');
  return [
    scalar('title', product.name), scalar('sku', sku),
    scalar('description', product.description), scalar('price', product.price),
    scalar('stock_on_hand', String(4 + ((index * 7) % 47))), scalar('available', index % 9 !== 0),
    scalar('launch_code', product.launchCode), scalar('available_on', `2026-${month}-${day}`),
    scalar('released_at', `2026-${month}-${day}T10:30:00Z`),
    scalar('order_cutoff', { time: '16:30:00', time_zone: 'America/New_York' }),
    scalar('meta_title', `${product.name} | ${storeName}`),
    scalar('meta_description', `${product.description} Available in ${product.color.name.toLowerCase()}.`),
  ];
};

const run = async () => {
  if (!catalogToken) throw new Error('Set CATALOG_TOKEN to a personal access token before running the generator.');
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
    ['Clothing', 'clothing', null], ["Women's Clothing", 'womens-clothing', 'Clothing'],
    ["Men's Clothing", 'mens-clothing', 'Clothing'], ['Footwear', 'footwear', null],
    ['Bags & Accessories', 'bags-accessories', null], ['Outerwear', 'outerwear', 'Clothing'],
    ['Activewear', 'activewear', 'Clothing'],
  ].map(async ([name, slug, parent]) => ({
    id: (await createEntity('category', [scalar('name', name), scalar('slug', slug)])).id,
    name,
    parent,
  })));
  await Promise.all(categories.flatMap(({ id, parent }) => {
    const parentId = categories.find((category) => category.name === parent)?.id;
    return parentId ? [replaceRelationships(id, [
      { attribute_code: 'parent_category', context_id: defaultContextId, target_entity_ids: [parentId] },
    ])] : [];
  }));
  const colorIds = new Map(await Promise.all(catalogColors.map(async ({ name, hex }) => [
    name, (await createEntity('color', [scalar('name', name), scalar('hex', hex)])).id,
  ])));

  let variantCount = 0;
  for (let index = 1; index <= productCount; index += 1) {
    const product = productForIndex(index);
    const parent = await createEntity('product', productValues(product, product.sku, index));
    const variants = [];
    for (const [variantIndex, size] of product.sizes.entries()) {
      const variant = {
        ...product,
        name: `${product.name} — Size ${size}`,
        sku: `${product.sku}-${String(variantIndex + 1).padStart(2, '0')}`,
      };
      variants.push(await createEntity(
        'product', productValues(variant, variant.sku, index * 10 + variantIndex),
      ));
    }
    variantCount += variants.length;
    const categoryId = categories.find((category) => category.name === product.category).id;
    const colorId = colorIds.get(product.color.name);
    await replaceRelationships(parent.id, [
      { attribute_code: 'categories', context_id: defaultContextId, target_entity_ids: [categoryId] },
      { attribute_code: 'colors', context_id: defaultContextId, target_entity_ids: [colorId] },
      { attribute_code: 'variants', context_id: defaultContextId, target_entity_ids: variants.map(({ id }) => id) },
    ]);
    for (const variant of variants) {
      await replaceRelationships(variant.id, [
        { attribute_code: 'categories', context_id: defaultContextId, target_entity_ids: [categoryId] },
        { attribute_code: 'colors', context_id: defaultContextId, target_entity_ids: [colorId] },
      ]);
    }
    if (index % 10 === 0) console.log(`Created ${index}/${productCount} parent products`);
  }

  const contextProduct = productForIndex(19);
  const sample = await createEntity('product', productValues(contextProduct, 'AR-BAG-STU-BLK-CONTEXT', 999));
  await replaceRelationships(sample.id, [
    { attribute_code: 'categories', context_id: defaultContextId, target_entity_ids: [categories.find((category) => category.name === contextProduct.category).id] },
    { attribute_code: 'colors', context_id: defaultContextId, target_entity_ids: [colorIds.get(contextProduct.color.name)] },
  ]);
  await request(`/entities/${sample.id}/values`, { method: 'POST', body: JSON.stringify({ values: [
    scalar('title', `${contextProduct.name} — US`, regional.id), scalar('price', 335, regional.id),
    scalar('title', `${contextProduct.name} — US Online`, web.id),
  ] }) });
  console.log(`Created ${productCount} parent products, ${variantCount} size variants, ${categories.length} categories, and ${colorIds.size} colors.`);
};

run().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});

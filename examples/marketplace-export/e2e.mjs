#!/usr/bin/env node
// End-to-end check of the export platform (#424) against this worktree's
// running stack and the local marketplace mock. It side-loads the test
// exporter, creates host-managed connections (client credentials, a refresh
// token and the device-code grant), publishes listings with variants and an
// image, runs connector jobs and checks what the mock received. It needs:
//
//   node examples/marketplace-export/mock-server/server.mjs   # separately
//   source .worktree && node examples/marketplace-export/e2e.mjs
//
// The API must run with EXTENSION_DEV_EGRESS_RESOLVE, EXTENSION_DEV_EGRESS_CA_FILE
// and EXTENSION_PUBLIC_FILE_BASE_URL set; see the README next to this file.

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const root = fileURLToPath(new URL('../../', import.meta.url));
const here = fileURLToPath(new URL('./', import.meta.url));
const env = Object.fromEntries(
  readFileSync(`${root}.env`, 'utf8')
    .split('\n')
    .filter((line) => /^[A-Z_]+=/.test(line))
    .map((line) => {
      const index = line.indexOf('=');
      return [line.slice(0, index), line.slice(index + 1).replace(/^"|"$/g, '')];
    }),
);
const apiUrl = process.env.ATTRICAT_API_URL ?? env.ATTRICAT_API_URL;
const mockUrl = process.env.MOCK_INSPECT_URL ?? 'http://127.0.0.1:9080';
const extensionId = 'attricat.marketplace-export-test';
const suffix = Date.now().toString(36);
const step = (message) => console.log(`\n▸ ${message}`);
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// ---- HTTP helpers -------------------------------------------------------------

const cookies = {};
const api = async (method, path, body, { expect = [200, 201, 202, 204], raw } = {}) => {
  const headers = {
    cookie: Object.entries(cookies).map(([name, value]) => `${name}=${value}`).join('; '),
  };
  if (cookies.attricat_csrf) headers['x-attricat-csrf'] = cookies.attricat_csrf;
  let payload;
  if (raw) {
    payload = raw.body;
    // FormData sets its own multipart boundary.
    if (raw.contentType) headers['content-type'] = raw.contentType;
  } else if (body !== undefined) {
    payload = JSON.stringify(body);
    headers['content-type'] = 'application/json';
  }
  const response = await fetch(`${apiUrl}${path}`, { method, headers, body: payload });
  for (const cookie of response.headers.getSetCookie()) {
    const [pair] = cookie.split(';');
    const [name, value] = pair.split('=');
    cookies[name] = value;
  }
  const text = await response.text();
  const parsed = text ? (() => { try { return JSON.parse(text); } catch { return text; } })() : null;
  if (!expect.includes(response.status)) {
    throw new Error(`${method} ${path} -> ${response.status}: ${text.slice(0, 500)}`);
  }
  return parsed;
};
const mock = async (path, body) => {
  const response = await fetch(`${mockUrl}${path}`, {
    method: body === undefined ? 'GET' : 'POST',
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  return response.json();
};

const waitFor = async (label, check, timeoutMs = 120_000) => {
  const started = Date.now();
  for (;;) {
    const result = await check();
    if (result) return result;
    if (Date.now() - started > timeoutMs) throw new Error(`timed out waiting for ${label}`);
    await sleep(500);
  }
};

const runJobs = async (jobs, key) => {
  const runIds = [];
  for (const job of jobs) {
    const result = await api('POST', `/blueprint-connector-jobs/${job.id}/run`, { idempotency_key: `${key}-${job.code}`.slice(0, 40) });
    runIds.push(...result.run_ids.map((id) => ({ id, job: job.code })));
  }
  return waitRuns(runIds);
};

const waitRuns = async (runIds) => {
  const runs = [];
  for (const { id, job } of runIds) {
    const run = await waitFor(`run ${job}`, async () => {
      const current = await api('GET', `/extension-operation-runs/${id}`);
      return ['completed', 'dead_letter', 'cancelled'].includes(current.status) ? current : undefined;
    });
    runs.push({ ...run, job });
    console.log(`  ${job}: ${run.status} ${JSON.stringify(run.progress?.stats ?? run.progress)}`);
  }
  return runs;
};

const exportStatus = (recordId) => api('GET', `/v1/records/${recordId}/export-status`);
const statusAt = async (recordId, connection) =>
  (await exportStatus(recordId)).statuses.find((mapping) => mapping.connection === connection);

// ---- scenario -------------------------------------------------------------------

step('reset the mock and sign in');
await mock('/reset', {});
const owner = env.ATTRICAT_BOOTSTRAP_OWNER_EMAIL ?? 'owner@example.test';
const password = readFileSync(`${root}.secrets/bootstrap_owner_password`, 'utf8').trim();
await api('POST', '/auth/login', { login_identifier: 'default.local', email: owner, password });

step('side-load the test exporter, grant it and enable it');
const installed = await api('GET', '/extensions');
if (installed.some((item) => item.extension_id === extensionId)) {
  for (const connection of await api('GET', `/extensions/${extensionId}/connections`)) {
    await api('DELETE', `/extensions/${extensionId}/connections/${connection.connection}`);
  }
  await api('POST', `/extensions/${extensionId}/disable`, undefined, { expect: [200, 409, 422] });
  await api('DELETE', `/extensions/${extensionId}`);
}
await api('POST', '/extensions/sideload', undefined, {
  raw: { body: readFileSync(`${here}dist/marketplace-export-test.tar.zst`), contentType: 'application/zstd' },
});
const manifest = JSON.parse(readFileSync(`${here}extension/manifest.json`, 'utf8'));
for (const capability of manifest.permissions) {
  await api('POST', `/extensions/${extensionId}/grants`, { grant_kind: 'capability', grant_id: capability });
}
for (const rule of manifest.host_permissions) {
  await api('POST', `/extensions/${extensionId}/grants`, { grant_kind: 'host_permission', grant_id: rule.id });
}
await api('PUT', `/extensions/${extensionId}/configure`, { configuration: {} });
await api('POST', `/extensions/${extensionId}/enable`);

step('create host-managed connections');
const connection = (input) => api('POST', `/extensions/${extensionId}/connections`, input);
const shopify = await connection({
  connection: 'shopify-eu', kind: 'shopify', display_name: 'Shopify EU', grant_type: 'client_credentials',
  token_url: 'https://shop.mock.test/admin/oauth/access_token', client_id: 'shopify-client', client_secret: 'shopify-secret',
  parameters: { api_base: 'https://shop.mock.test', api_version: '2026-10' },
});
assert.equal(shopify.status, 'active');
for (const name of ['amazon-de', 'amazon-feed']) {
  await connection({
    connection: name, kind: 'amazon', display_name: name, grant_type: 'refresh_token',
    token_url: 'https://lwa.mock.test/auth/o2/token', client_id: 'amzn-client', client_secret: 'amzn-secret',
    refresh_token: 'Atzr|initial-refresh-token',
    parameters: { seller_id: 'A1SELLER', marketplace_id: 'A1PA6795UKMFR9', currency: 'EUR' },
  });
}
const allegro = await connection({
  connection: 'allegro-pl', kind: 'allegro', display_name: 'Allegro PL', grant_type: 'device_code',
  token_url: 'https://allegro.mock.test/auth/oauth/token', device_authorization_url: 'https://allegro.mock.test/auth/oauth/device',
  client_id: 'allegro-client', client_secret: 'allegro-secret', parameters: { currency: 'PLN' },
});
assert.equal(allegro.status, 'pending_authorization');
const device = await api('POST', `/extensions/${extensionId}/connections/allegro-pl/device-authorization`);
console.log(`  Allegro asks the operator to enter ${device.device_user_code} at ${device.device_verification_uri}`);
assert.equal((await api('POST', `/extensions/${extensionId}/connections/allegro-pl/device-authorization/poll`)).status, 'pending_authorization');
await mock('/allegro/approve', { user_code: device.device_user_code });
// The host polls the token endpoint no faster than the device interval (5 s).
await waitFor('Allegro device authorization', async () => {
  const polled = await api('POST', `/extensions/${extensionId}/connections/allegro-pl/device-authorization/poll`);
  return polled.status === 'active' ? polled : undefined;
}, 30_000);
for (const name of ['shopify-eu', 'amazon-de']) {
  await api('POST', `/extensions/${extensionId}/connections/${name}/test`);
}
const listed = JSON.stringify(await api('GET', `/extensions/${extensionId}/connections`));
for (const secret of ['shopify-secret', 'amzn-secret', 'allegro-secret', 'Atzr|initial', 'shpat-', 'Atza-', 'allegro-access', 'allegro-refresh']) {
  assert(!listed.includes(secret), `connection listing leaked ${secret}`);
}

step('create a publication channel and listing blueprints with export jobs');
// Jobs run once per enabled channel; switch off channels of earlier runs.
for (const existing of await api('GET', '/publication-channels')) {
  if (existing.enabled && existing.context_code.startsWith('marketplaces_')) {
    await api('PUT', `/publication-channels/${existing.context_id}`, { enabled: false });
  }
}
const contexts = await api('GET', '/contexts');
const defaultContext = contexts.find((context) => context.code === 'default');
const channel = await api('POST', '/contexts', { code: `marketplaces_${suffix}`, data: {}, parent_id: defaultContext.id });
await api('PUT', `/publication-channels/${channel.id}`, { enabled: true });
const variantCode = `mx_variant_${suffix}`;
const listingCode = `mx_listing_${suffix}`;
const createBlueprint = async (definition) => {
  const created = await api('POST', '/blueprints', { definition });
  await api('POST', `/blueprints/${created.blueprint.id}/versions/${created.blueprint.version}/publish`);
  return created.blueprint;
};
await createBlueprint(`format_version = 1
code = "${variantCode}"
name = "Marketplace variant ${suffix}"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["sku"]
[[attributes]]
code = "sku"
value_type = "string"
[[attributes]]
code = "size"
value_type = "string"
[[attributes]]
code = "price"
value_type = "number"
`);
const job = (code, connectionName, input = '{}') => `
[[connector_jobs]]
code = "${code}"
direction = "export"
extension_id = "${extensionId}"
operation_id = "export"
connection = "${connectionName}"
input = ${input}
`;
const listingBlueprint = await createBlueprint(`format_version = 1
code = "${listingCode}"
name = "Marketplace listing ${suffix}"
kind = "record"
[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]
[[attributes]]
code = "sku"
value_type = "string"
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "description"
value_type = "string"
[[attributes]]
code = "price"
value_type = "number"
[[attributes]]
code = "image"
value_type = "file"
allowed_mime_groups = ["image/png"]
allowed_extensions = ["png"]
max_bytes = 1048576
[[attributes]]
code = "variants"
value_type = "relationship"
target_blueprint = "${variantCode}"
cardinality = "many"
${job('shopify', 'shopify-eu')}${job('amazon_items', 'amazon-de')}${job('amazon_feed', 'amazon-feed', '{feed = true}')}${job('allegro', 'allegro-pl')}`);
const jobs = await api('GET', `/blueprints/${listingBlueprint.id}/connector-jobs`);
assert.equal(jobs.length, 4);

step('create listings with variants and an image, and publish them');
const scalar = (code, value) => ({ kind: 'scalar', attribute_code: code, value });
const record = async (blueprint, values) => (await api('POST', '/v1/records', { blueprint: { code: blueprint }, values })).id;
const publish = (id) => api('POST', `/v1/records/${id}/publications`, { context_id: channel.id });
const small = record.bind(null, variantCode);
const variantS = await small([scalar('sku', `SHIRT-${suffix}-S`), scalar('size', 'S'), scalar('price', 19.99)]);
const variantM = await small([scalar('sku', `SHIRT-${suffix}-M`), scalar('size', 'M'), scalar('price', 21.99)]);
const shirt = await record(listingCode, [
  scalar('sku', `SHIRT-${suffix}`), scalar('title', 'Linen shirt'), scalar('description', 'Breathable linen'), scalar('price', 19.99),
  { kind: 'relationship', attribute_code: 'variants', target_record_id: variantS },
  { kind: 'relationship', attribute_code: 'variants', target_record_id: variantM },
]);
const png = Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==', 'base64');
const upload = new FormData();
upload.append('file', new Blob([png], { type: 'image/png' }), 'shirt.png');
await api('POST', `/records/${shirt}/file-attributes/image/uploads`, undefined, { raw: { body: upload } });
const mug = await record(listingCode, [scalar('sku', `MUG-${suffix}`), scalar('title', 'Stoneware mug'), scalar('description', 'Dishwasher safe'), scalar('price', 9.5)]);
const broken = await record(listingCode, [scalar('sku', `BROKEN-${suffix}`), scalar('description', 'No title, no price')]);
for (const id of [variantS, variantM, shirt, mug, broken]) {
  await publish(id);
}

step('the export_mapping frame sets the Amazon product type');
const mappingFrame = (await api('GET', '/extensions/runtime')).find(
  (item) => item.extension_id === extensionId && item.outlet === 'export_mapping',
);
assert.equal(mappingFrame?.kind, 'panel', 'export_mapping contribution is offered to the host');
const frameSource = await api('GET', `/extensions/${extensionId}/${mappingFrame.id}/artifact`);
assert(String(frameSource).includes('export const mount'), 'export_mapping artifact is served');
// Unlike other panels, the mapping editor may call the extension's commands.
const fieldMap = (payload) => api('POST', `/extensions/${extensionId}/${mappingFrame.id}/command`,
  { release_id: mappingFrame.release_id, command_id: 'field-map', payload: { export_id: 'amazon', ...payload } });
assert.match((await fieldMap({ product_type: 'not valid' })).error, /capital letters/);
assert.equal((await fieldMap({ product_type: 'SHIRT' })).saved, true);
assert.equal((await fieldMap({})).product_type, 'SHIRT');

step('preview a payload without pushing');
const preview = await api('POST', `/extensions/${extensionId}/exports/amazon/preview`, { record_id: broken, connection: 'amazon-de' });
console.log(`  preview issues: ${preview.issues.join('; ')}`);
assert(preview.issues.includes('title is required'));
assert.equal(preview.payload.productType, 'SHIRT', 'preview uses the mapped product type');

step('run every export job');
const firstRuns = await runJobs(jobs, `first-${suffix}`);
for (const run of firstRuns) assert.equal(run.status, 'completed', `${run.job} did not complete`);
let state = await mock('/state');
const product = Object.values(state.shopify.products).find((item) => item.handle === `shirt-${suffix}`);
assert(product, 'Shopify product exists');
assert.equal(product.variants.length, 2, 'Shopify variants');
assert.equal(product.media[0]?.status, 'READY', 'Shopify media staged and uploaded');
assert(state.amazon.listings[`SHIRT-${suffix}-M`]?.attributes.child_parent_sku_relationship, 'Amazon child listing');
assert.equal(Object.values(state.amazon.feeds)[0]?.status, 'DONE', 'Amazon feed processed');
assert.equal(state.amazon.listings[`SHIRT-${suffix}-M`]?.productType, 'SHIRT', 'Amazon listing uses the mapped product type');
const offer = Object.values(state.allegro.offers).find((item) => item.external?.id === `SHIRT-${suffix}`);
assert.equal(offer?.publication.status, 'ACTIVE', 'Allegro offer active after polling');
await waitFor('marketplaces to fetch the public image link', async () => {
  const current = await mock('/state');
  return current.amazon.listings[`SHIRT-${suffix}`]?.image?.status === 200 ? current : undefined;
}, 15_000);
for (const name of ['shopify-eu', 'amazon-de', 'amazon-feed', 'allegro-pl']) {
  assert.equal((await statusAt(shirt, name))?.status, 'synced', `shirt at ${name}`);
  const rejected = await statusAt(broken, name);
  assert.equal(rejected?.status, 'rejected', `broken listing at ${name}`);
  console.log(`  broken at ${name}: ${rejected.error.code} ${rejected.error.message}`);
}
assert.equal((await api('GET', `/extensions/${extensionId}`)).installation.state, 'enabled');

step('throttling and a temporary 5xx defer or retry without quarantine');
await mock('/fail', { host: 'shop.mock.test', path: '/admin/api', status: 429, retryAfter: 1 });
await mock('/fail', { host: 'allegro.mock.test', path: '/sale/product-offers', status: 503, retryAfter: 1 });
await api('POST', `/records/${mug}/values`, { values: [scalar('price', 11.25)] }, { expect: [200, 201] });
await publish(mug);
const throttledRuns = await runJobs(jobs.filter((item) => ['shopify', 'allegro'].includes(item.code)), `throttle-${suffix}`);
for (const run of throttledRuns) assert.equal(run.status, 'completed', `${run.job} after a throttle/5xx`);
state = await mock('/state');
assert.equal(Object.values(state.shopify.products).find((item) => item.handle === `mug-${suffix}`)?.variants[0].price, '11.25');
assert.equal((await api('GET', `/extensions/${extensionId}`)).installation.state, 'enabled');

step('an uncertain push is reconciled by SKU before pushing again');
await mock('/fail', { host: 'sellingpartnerapi.mock.test', path: `/listings/2021-08-01/items/A1SELLER/MUG-${suffix}`, mode: 'hang', delayMs: 6000 });
await api('POST', `/records/${mug}/values`, { values: [scalar('description', 'Dishwasher and microwave safe')] }, { expect: [200, 201] });
await publish(mug);
const amazonItems = jobs.filter((item) => item.code === 'amazon_items');
await runJobs(amazonItems, `uncertain-${suffix}`);
assert.equal((await statusAt(mug, 'amazon-de'))?.status, 'uncertain');
await api('POST', `/extensions/${extensionId}/export-status/retry`, { connection: 'amazon-de', record_ids: [mug], run: true }, { expect: [202] })
  .then((result) => waitRuns(result.run_ids.map((id) => ({ id, job: 'amazon_items retry' }))));
assert.equal((await statusAt(mug, 'amazon-de'))?.status, 'synced');

step('a fixed record is retried on its own');
await api('POST', `/records/${broken}/values`, { values: [scalar('title', 'Fixed listing'), scalar('price', 5)] }, { expect: [200, 201] });
await publish(broken);
const retried = await api('POST', `/extensions/${extensionId}/export-status/retry`, { connection: 'shopify-eu', record_ids: [broken], run: true }, { expect: [202] });
await waitRuns(retried.run_ids.map((id) => ({ id, job: 'shopify retry' })));
assert.equal((await statusAt(broken, 'shopify-eu'))?.status, 'synced');

step('unpublishing withdraws items on every destination');
await api('POST', `/v1/records/${mug}/publications/unpublish`, { context_id: channel.id });
const withdrawRuns = await runJobs(jobs, `withdraw-${suffix}`);
for (const run of withdrawRuns) assert.equal(run.status, 'completed');
state = await mock('/state');
assert(!Object.values(state.shopify.products).some((item) => item.handle === `mug-${suffix}`), 'Shopify product deleted');
assert(!state.amazon.listings[`MUG-${suffix}`], 'Amazon listing deleted');
const mugOffers = Object.values(state.allegro.offers).filter((item) => item.external?.id === `MUG-${suffix}`);
assert.equal(mugOffers.length, 1, 'an update changes the mapped offer instead of creating another');
assert.equal(mugOffers[0].publication.status, 'ENDED');
for (const name of ['shopify-eu', 'amazon-de', 'allegro-pl']) {
  assert.equal((await statusAt(mug, name))?.status, 'withdrawn', `mug at ${name}`);
}

step('a second run pushes nothing that did not change');
const quietRuns = await runJobs(jobs.filter((item) => item.code === 'shopify'), `quiet-${suffix}`);
assert.equal(quietRuns[0].progress.stats.synced ?? 0, 0);

step('export health and secrets');
const health = await api('GET', '/export-status');
console.log(`  ${health.counts.filter((count) => count.extension_id === extensionId).map((count) => `${count.connection}:${count.status}=${count.count}`).join(' ')}`);
assert.equal((await mock('/secrets-check')).leaked.length, 0);
assert.equal((await mock('/state')).leaks.length, 0, 'no refresh token was used twice');
const log = readFileSync(process.env.API_LOG ?? '/dev/null', 'utf8');
for (const secret of ['shopify-secret', 'amzn-secret', 'allegro-secret', 'Atzr|initial', 'shpat-', 'Atza-', 'allegro-access-', 'allegro-refresh-']) {
  assert(!log.includes(secret), `API log contains ${secret}`);
}
console.log('\n✓ marketplace export end-to-end check passed');

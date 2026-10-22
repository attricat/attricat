#!/usr/bin/env node
// A local stand-in for the Shopify Admin GraphQL API, Amazon Selling Partner
// API (with Login with Amazon and S3 feed documents) and the Allegro REST API.
// It never talks to the real services. It serves HTTPS for `*.mock.test`
// with the test CA in apps/api/tests/fixtures/mock-tls, so Attricat's
// extension broker reaches it through EXTENSION_DEV_EGRESS_RESOLVE and
// EXTENSION_DEV_EGRESS_CA_FILE, and an HTTP inspection API on another port.
//
//   node examples/marketplace-export/mock-server/server.mjs
//
// MOCK_HTTPS_PORT (default 9443) and MOCK_INSPECT_PORT (default 9080) choose
// the ports. The inspection API serves `/` (an HTML overview), `/state`,
// `/requests`, `POST /reset`, `POST /fail` and `POST /allegro/approve`.

import { randomUUID, createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import http from 'node:http';
import https from 'node:https';
import { fileURLToPath } from 'node:url';

const fixtures = fileURLToPath(
  new URL('../../../apps/api/tests/fixtures/mock-tls/', import.meta.url),
);
const httpsPort = Number(process.env.MOCK_HTTPS_PORT ?? 9443);
const inspectPort = Number(process.env.MOCK_INSPECT_PORT ?? 9080);

/** Credentials the operator enters in Attricat connections. */
export const credentials = {
  shopify: { clientId: 'shopify-client', clientSecret: 'shopify-secret' },
  amazon: {
    clientId: 'amzn-client',
    clientSecret: 'amzn-secret',
    refreshToken: 'Atzr|initial-refresh-token',
  },
  allegro: { clientId: 'allegro-client', clientSecret: 'allegro-secret' },
};

const freshState = () => ({
  shopify: { tokens: new Set(), products: {}, staged: {}, nextId: 1 },
  amazon: {
    tokens: new Set(),
    refreshTokens: new Set([credentials.amazon.refreshToken]),
    listings: {},
    documents: {},
    feeds: {},
    s3: {},
  },
  allegro: {
    tokens: new Set(),
    refreshTokens: new Map(),
    devices: {},
    offers: {},
    operations: {},
  },
  requests: [],
  injections: [],
  leaks: [],
});
let state = freshState();

const json = (response, status, body, headers = {}) => {
  response.writeHead(status, { 'content-type': 'application/json', ...headers });
  response.end(JSON.stringify(body));
};

const readBody = (request) =>
  new Promise((resolve) => {
    const chunks = [];
    request.on('data', (chunk) => chunks.push(chunk));
    request.on('end', () => resolve(Buffer.concat(chunks)));
  });

const form = (buffer) => Object.fromEntries(new URLSearchParams(buffer.toString()));

const basic = (request) => {
  const header = request.headers.authorization ?? '';
  if (!header.startsWith('Basic ')) return {};
  const [id, secret] = Buffer.from(header.slice(6), 'base64').toString().split(':');
  return { id, secret };
};

const bearer = (request) => {
  const header = request.headers.authorization ?? '';
  return header.startsWith('Bearer ') ? header.slice(7) : undefined;
};

const issue = (prefix) => `${prefix}-${randomUUID()}`;

/** Marketplaces fetch listing images themselves, asynchronously. */
const fetchImage = (record, url) => {
  if (!url) return;
  record.image = { url, status: 'fetching' };
  fetch(url)
    .then(async (response) => {
      const bytes = (await response.arrayBuffer()).byteLength;
      record.image = { url, status: response.status, bytes };
    })
    .catch((error) => {
      record.image = { url, status: 'failed', error: String(error) };
    });
};

/** A request is answered with an injected failure once per configured time. */
const injected = (host, path) => {
  const injection = state.injections.find(
    (item) => item.host === host && path.startsWith(item.path) && item.times > 0,
  );
  if (!injection) return undefined;
  injection.times -= 1;
  return injection;
};

/** Parses a multipart/form-data body into fields and one file. */
const multipart = (buffer, contentType) => {
  const boundary = /boundary=([^;]+)/.exec(contentType ?? '')?.[1];
  if (!boundary) return undefined;
  const fields = {};
  let file;
  for (const part of buffer.toString('latin1').split(`--${boundary}`)) {
    const [head, ...rest] = part.split('\r\n\r\n');
    const name = /name="([^"]+)"/.exec(head ?? '')?.[1];
    if (!name) continue;
    const value = rest.join('\r\n\r\n').replace(/\r\n$/, '');
    const filename = /filename="([^"]+)"/.exec(head)?.[1];
    if (filename) {
      file = {
        name,
        filename,
        contentType: /content-type:\s*([^\r\n]+)/i.exec(head)?.[1],
        bytes: Buffer.from(value, 'latin1').length,
      };
    } else {
      fields[name] = value;
    }
  }
  return { fields, file };
};

// ---- Shopify --------------------------------------------------------------

const shopify = async (request, response, url, body) => {
  if (url.pathname === '/admin/oauth/access_token' && request.method === 'POST') {
    const input = form(body);
    if (
      input.grant_type !== 'client_credentials' ||
      input.client_id !== credentials.shopify.clientId ||
      input.client_secret !== credentials.shopify.clientSecret
    ) {
      return json(response, 401, { error: 'invalid_client' });
    }
    const token = issue('shpat');
    state.shopify.tokens.add(token);
    return json(response, 200, {
      access_token: token,
      scope: 'write_products',
      expires_in: 86399,
    });
  }
  if (!url.pathname.startsWith('/admin/api/') || !url.pathname.endsWith('/graphql.json')) {
    return json(response, 404, { errors: 'Not Found' });
  }
  if (!state.shopify.tokens.has(request.headers['x-shopify-access-token'])) {
    return json(response, 401, { errors: '[API] Invalid API key or access token' });
  }
  const { query = '', variables = {} } = JSON.parse(body.toString() || '{}');
  if (query.includes('stagedUploadsCreate')) {
    const targets = (variables.input ?? []).map((input) => {
      const key = `tmp/${randomUUID()}/${input.filename}`;
      state.shopify.staged[key] = { mimeType: input.mimeType, uploaded: false };
      return {
        url: 'https://storage.mock.test/upload',
        resourceUrl: `https://storage.mock.test/upload/${key}`,
        parameters: [
          { name: 'key', value: key },
          { name: 'Content-Type', value: input.mimeType },
          { name: 'policy', value: Buffer.from(key).toString('base64') },
          { name: 'x-goog-signature', value: createHash('sha256').update(key).digest('hex') },
        ],
      };
    });
    return json(response, 200, {
      data: { stagedUploadsCreate: { stagedTargets: targets, userErrors: [] } },
    });
  }
  if (query.includes('productSet')) {
    const input = variables.input ?? {};
    if (!input.title?.trim()) {
      return json(response, 200, {
        data: {
          productSet: {
            product: null,
            userErrors: [{ field: ['input', 'title'], message: "Title can't be blank", code: 'BLANK' }],
          },
        },
      });
    }
    const existing = Object.values(state.shopify.products).find(
      (product) =>
        (variables.identifier?.id && product.id === variables.identifier.id) ||
        (variables.identifier?.handle && product.handle === variables.identifier.handle),
    );
    const id = existing?.id ?? `gid://shopify/Product/${state.shopify.nextId++}`;
    const media = (input.files ?? []).map((file) => {
      const key = file.originalSource.replace('https://storage.mock.test/upload/', '');
      const staged = state.shopify.staged[key];
      return {
        source: file.originalSource,
        status: staged?.uploaded || !staged ? 'READY' : 'FAILED',
      };
    });
    const product = {
      id,
      handle: input.handle ?? variables.identifier?.handle,
      title: input.title,
      descriptionHtml: input.descriptionHtml ?? '',
      status: input.status ?? 'ACTIVE',
      variants: (input.variants ?? []).map((variant, index) => ({
        id: `gid://shopify/ProductVariant/${id.split('/').pop()}${index + 1}`,
        sku: variant.sku,
        price: variant.price,
        options: (variant.optionValues ?? []).map((option) => option.name),
      })),
      media,
      updatedAt: new Date().toISOString(),
    };
    state.shopify.products[id] = product;
    return json(response, 200, {
      data: {
        productSet: {
          product: {
            id,
            variants: { nodes: product.variants.map(({ id, sku }) => ({ id, sku })) },
          },
          userErrors: [],
        },
      },
    });
  }
  if (query.includes('productDelete')) {
    const id = variables.input?.id;
    const existed = Boolean(state.shopify.products[id]);
    delete state.shopify.products[id];
    return json(response, 200, {
      data: { productDelete: { deletedProductId: existed ? id : null, userErrors: [] } },
    });
  }
  if (query.includes('shop')) {
    return json(response, 200, { data: { shop: { name: 'Mock shop' } } });
  }
  return json(response, 200, { errors: [{ message: 'Unsupported operation in the mock' }] });
};

const storage = async (request, response, url, body) => {
  if (request.method !== 'POST' || url.pathname !== '/upload') {
    return json(response, 404, {});
  }
  const parsed = multipart(body, request.headers['content-type']);
  const key = parsed?.fields.key;
  const staged = key && state.shopify.staged[key];
  if (!staged || !parsed.file || !parsed.fields['x-goog-signature']) {
    response.writeHead(403, { 'content-type': 'application/xml' });
    return response.end('<Error><Code>AccessDenied</Code></Error>');
  }
  staged.uploaded = true;
  staged.bytes = parsed.file.bytes;
  staged.contentType = parsed.file.contentType;
  response.writeHead(204);
  response.end();
};

// ---- Amazon ---------------------------------------------------------------

const lwa = async (request, response, url, body) => {
  if (url.pathname !== '/auth/o2/token' || request.method !== 'POST') {
    return json(response, 404, {});
  }
  const input = form(body);
  if (
    input.grant_type !== 'refresh_token' ||
    input.client_id !== credentials.amazon.clientId ||
    input.client_secret !== credentials.amazon.clientSecret ||
    !state.amazon.refreshTokens.has(input.refresh_token)
  ) {
    return json(response, 400, { error: 'invalid_grant', error_description: 'Grant rejected' });
  }
  const token = issue('Atza');
  state.amazon.tokens.add(token);
  return json(response, 200, { access_token: token, token_type: 'bearer', expires_in: 3600 });
};

/** Validates SP-API listing attributes like the Listings Items API. */
const listingIssues = (body) => {
  const issues = [];
  if (!body.attributes?.item_name?.[0]?.value) {
    issues.push({ code: '90220', message: "'item_name' is required but not supplied.", severity: 'ERROR', attributeNames: ['item_name'] });
  }
  if (!body.attributes?.purchasable_offer?.[0]?.our_price?.[0]?.schedule?.[0]?.value_with_tax) {
    issues.push({ code: '90220', message: "'purchasable_offer' is required but not supplied.", severity: 'ERROR', attributeNames: ['purchasable_offer'] });
  }
  return issues;
};

const presigned = (host, path) =>
  `https://${host}${path}?X-Amz-Algorithm=AWS4-HMAC-SHA256&X-Amz-Expires=300&X-Amz-Signature=${createHash('sha256').update(path).digest('hex')}`;

const spApi = async (request, response, url, body) => {
  if (!state.amazon.tokens.has(request.headers['x-amz-access-token'])) {
    return json(response, 403, { errors: [{ code: 'Unauthorized', message: 'Access to requested resource is denied.' }] });
  }
  const limitHeaders = { 'x-amzn-ratelimit-limit': '5.0', 'x-amzn-requestid': randomUUID() };
  const listing = /^\/listings\/2021-08-01\/items\/([^/]+)\/([^/]+)$/.exec(url.pathname);
  if (listing) {
    if (!url.searchParams.get('marketplaceIds')) {
      return json(response, 400, { errors: [{ code: 'InvalidInput', message: 'marketplaceIds is required' }] }, limitHeaders);
    }
    const sku = decodeURIComponent(listing[2]);
    if (request.method === 'GET') {
      const item = state.amazon.listings[sku];
      return item
        ? json(response, 200, { sku, summaries: [{ status: ['BUYABLE'], itemName: item.attributes.item_name[0].value }] }, limitHeaders)
        : json(response, 404, { errors: [{ code: 'NOT_FOUND', message: 'SKU not found' }] }, limitHeaders);
    }
    if (request.method === 'DELETE') {
      delete state.amazon.listings[sku];
      return json(response, 200, { sku, status: 'ACCEPTED', submissionId: randomUUID(), issues: [] }, limitHeaders);
    }
    if (request.method === 'PUT') {
      const input = JSON.parse(body.toString() || '{}');
      const issues = listingIssues(input);
      if (issues.length) {
        return json(response, 200, { sku, status: 'INVALID', submissionId: randomUUID(), issues }, limitHeaders);
      }
      const item = { ...input, sellerId: listing[1], via: 'listings', updatedAt: new Date().toISOString() };
      state.amazon.listings[sku] = item;
      fetchImage(item, input.attributes?.main_product_image_locator?.[0]?.media_location);
      return json(response, 200, { sku, status: 'ACCEPTED', submissionId: randomUUID(), issues: [] }, limitHeaders);
    }
  }
  if (url.pathname === '/feeds/2021-06-30/documents' && request.method === 'POST') {
    const input = JSON.parse(body.toString() || '{}');
    const id = `amzn1.tortuga.4.eu.${randomUUID()}`;
    state.amazon.documents[id] = { contentType: input.contentType, path: `/feeds/${id}` };
    return json(response, 201, { feedDocumentId: id, url: presigned('s3.mock.test', `/feeds/${id}`) });
  }
  if (url.pathname === '/feeds/2021-06-30/feeds' && request.method === 'POST') {
    const input = JSON.parse(body.toString() || '{}');
    const document = state.amazon.documents[input.inputFeedDocumentId];
    if (!document || !state.amazon.s3[document.path]) {
      return json(response, 400, { errors: [{ code: 'InvalidInput', message: 'Feed document was not uploaded' }] });
    }
    const id = String(Object.keys(state.amazon.feeds).length + 50001);
    state.amazon.feeds[id] = { feedType: input.feedType, documentId: input.inputFeedDocumentId, marketplaceIds: input.marketplaceIds, polls: 0, status: 'IN_QUEUE' };
    return json(response, 202, { feedId: id });
  }
  const feed = /^\/feeds\/2021-06-30\/feeds\/([^/]+)$/.exec(url.pathname);
  if (feed && request.method === 'GET') {
    const item = state.amazon.feeds[feed[1]];
    if (!item) return json(response, 404, {});
    item.polls += 1;
    if (item.polls === 1) item.status = 'IN_PROGRESS';
    if (item.polls >= 2 && item.status !== 'DONE') {
      // Processing applies every message and writes a result document.
      const upload = state.amazon.s3[state.amazon.documents[item.documentId].path];
      const feedJson = JSON.parse(upload.body);
      const issues = [];
      for (const message of feedJson.messages ?? []) {
        const problems = listingIssues(message);
        if (problems.length) {
          issues.push(...problems.map((problem) => ({ messageId: message.messageId, ...problem })));
        } else {
          const item = { productType: message.productType, attributes: message.attributes, via: 'feed', updatedAt: new Date().toISOString() };
          state.amazon.listings[message.sku] = item;
          fetchImage(item, message.attributes?.main_product_image_locator?.[0]?.media_location);
        }
      }
      const resultId = `amzn1.tortuga.4.eu.result.${randomUUID()}`;
      const resultPath = `/results/${resultId}`;
      state.amazon.s3[resultPath] = {
        contentType: 'application/json',
        body: JSON.stringify({
          header: feedJson.header,
          summary: {
            errors: issues.length,
            messagesProcessed: feedJson.messages?.length ?? 0,
            messagesAccepted: (feedJson.messages?.length ?? 0) - new Set(issues.map((issue) => issue.messageId)).size,
            messagesInvalid: new Set(issues.map((issue) => issue.messageId)).size,
          },
          issues,
        }),
      };
      state.amazon.documents[resultId] = { contentType: 'application/json', path: resultPath };
      item.status = 'DONE';
      item.resultFeedDocumentId = resultId;
    }
    return json(response, 200, {
      feedId: feed[1],
      feedType: item.feedType,
      processingStatus: item.status,
      ...(item.resultFeedDocumentId ? { resultFeedDocumentId: item.resultFeedDocumentId } : {}),
    });
  }
  const document = /^\/feeds\/2021-06-30\/documents\/([^/]+)$/.exec(url.pathname);
  if (document && request.method === 'GET') {
    const item = state.amazon.documents[document[1]];
    if (!item) return json(response, 404, {});
    return json(response, 200, { feedDocumentId: document[1], url: presigned('s3.mock.test', item.path) });
  }
  return json(response, 404, { errors: [{ code: 'NotFound', message: 'Unknown SP-API path in the mock' }] });
};

const s3 = async (request, response, url, body) => {
  if (!url.searchParams.get('X-Amz-Signature')) {
    response.writeHead(403, { 'content-type': 'application/xml' });
    return response.end('<Error><Code>AccessDenied</Code></Error>');
  }
  if (request.method === 'PUT') {
    const document = Object.values(state.amazon.documents).find((item) => item.path === url.pathname);
    // The signature covers the content type the document was created with.
    if (!document || request.headers['content-type'] !== document.contentType) {
      response.writeHead(403, { 'content-type': 'application/xml' });
      return response.end('<Error><Code>SignatureDoesNotMatch</Code></Error>');
    }
    state.amazon.s3[url.pathname] = { contentType: request.headers['content-type'], body: body.toString() };
    response.writeHead(200, { etag: `"${createHash('md5').update(body).digest('hex')}"` });
    return response.end();
  }
  if (request.method === 'GET') {
    const object = state.amazon.s3[url.pathname];
    if (!object) return json(response, 404, {});
    response.writeHead(200, { 'content-type': object.contentType, etag: `"${createHash('md5').update(object.body).digest('hex')}"` });
    return response.end(object.body);
  }
  return json(response, 405, {});
};

// ---- Allegro --------------------------------------------------------------

const allegroTokens = (refreshToken) => {
  const access = issue('allegro-access');
  const refresh = issue('allegro-refresh');
  state.allegro.tokens.add(access);
  state.allegro.refreshTokens.set(refresh, false);
  if (refreshToken) state.allegro.refreshTokens.set(refreshToken, true);
  return { access_token: access, refresh_token: refresh, token_type: 'bearer', expires_in: 43199 };
};

const allegro = async (request, response, url, body) => {
  if (url.pathname === '/auth/oauth/device' && request.method === 'POST') {
    const client = basic(request);
    if (client.id !== credentials.allegro.clientId || client.secret !== credentials.allegro.clientSecret) {
      return json(response, 401, { error: 'unauthorized' });
    }
    const deviceCode = issue('device');
    const userCode = Math.random().toString(36).slice(2, 10).toUpperCase();
    state.allegro.devices[deviceCode] = { userCode, approved: false };
    return json(response, 200, {
      device_code: deviceCode,
      user_code: userCode,
      verification_uri: 'https://allegro.mock.test/skojarz',
      verification_uri_complete: `https://allegro.mock.test/skojarz?code=${userCode}`,
      expires_in: 3600,
      interval: 5,
    });
  }
  if (url.pathname === '/auth/oauth/token' && request.method === 'POST') {
    const client = basic(request);
    if (client.id !== credentials.allegro.clientId || client.secret !== credentials.allegro.clientSecret) {
      return json(response, 401, { error: 'invalid_client' });
    }
    const input = form(body);
    if (input.grant_type === 'urn:ietf:params:oauth:grant-type:device_code') {
      const device = state.allegro.devices[input.device_code];
      if (!device) return json(response, 400, { error: 'invalid_grant' });
      if (!device.approved) return json(response, 400, { error: 'authorization_pending' });
      delete state.allegro.devices[input.device_code];
      return json(response, 200, allegroTokens());
    }
    if (input.grant_type === 'refresh_token') {
      // Rotating refresh tokens: each can be exchanged once.
      if (state.allegro.refreshTokens.get(input.refresh_token) !== false) {
        state.leaks.push({ at: new Date().toISOString(), what: 'reused or unknown Allegro refresh token' });
        return json(response, 400, { error: 'invalid_grant' });
      }
      return json(response, 200, allegroTokens(input.refresh_token));
    }
    return json(response, 400, { error: 'unsupported_grant_type' });
  }
  if (!state.allegro.tokens.has(bearer(request))) {
    return json(response, 401, { error: 'invalid_token' });
  }
  const media = { 'content-type': 'application/vnd.allegro.public.v1+json' };
  if (url.pathname === '/sale/product-offers' && request.method === 'POST') {
    const input = JSON.parse(body.toString() || '{}');
    const errors = [];
    if (!input.productSet?.[0]?.product?.name) errors.push({ code: 'VALIDATION_ERROR', message: 'Product name is required', path: 'productSet[0].product.name' });
    if (!input.sellingMode?.price?.amount) errors.push({ code: 'VALIDATION_ERROR', message: 'Price is required', path: 'sellingMode.price.amount' });
    if (errors.length) return json(response, 422, { errors }, media);
    const offerId = String(7_000_000_000 + Object.keys(state.allegro.offers).length + 1);
    const operationId = randomUUID();
    state.allegro.offers[offerId] = { id: offerId, name: input.productSet[0].product.name, external: input.external, price: input.sellingMode.price, stock: input.stock, publication: { status: 'INACTIVE' } };
    fetchImage(state.allegro.offers[offerId], input.productSet[0].product.images?.[0]);
    state.allegro.operations[operationId] = { offerId, polls: 0 };
    return json(response, 202, { id: offerId, ...state.allegro.offers[offerId] }, {
      ...media,
      location: `https://allegro.mock.test/sale/product-offers/${offerId}/operations/${operationId}`,
      'retry-after': '1',
    });
  }
  const operation = /^\/sale\/product-offers\/([^/]+)\/operations\/([^/]+)$/.exec(url.pathname);
  if (operation && request.method === 'GET') {
    const item = state.allegro.operations[operation[2]];
    if (!item) return json(response, 404, {}, media);
    item.polls += 1;
    if (item.polls < 2) {
      return json(response, 202, { id: item.offerId }, { ...media, 'retry-after': '1', location: `https://allegro.mock.test${url.pathname}` });
    }
    state.allegro.offers[item.offerId].publication = { status: 'ACTIVE' };
    // Like Allegro, a finished command redirects to the offer.
    response.writeHead(303, { location: `https://allegro.mock.test/sale/product-offers/${item.offerId}` });
    return response.end();
  }
  const offer = /^\/sale\/product-offers\/([^/]+)$/.exec(url.pathname);
  if (offer && request.method === 'GET') {
    const item = state.allegro.offers[offer[1]];
    return item ? json(response, 200, item, media) : json(response, 404, {}, media);
  }
  if (offer && request.method === 'PATCH') {
    const item = state.allegro.offers[offer[1]];
    if (!item) return json(response, 404, {}, media);
    const input = JSON.parse(body.toString() || '{}');
    if (input.sellingMode && !input.sellingMode.price?.amount) {
      return json(response, 422, { errors: [{ code: 'VALIDATION_ERROR', message: 'Price is required', path: 'sellingMode.price.amount' }] }, media);
    }
    if (input.productSet?.[0]?.product?.name) item.name = input.productSet[0].product.name;
    if (input.sellingMode?.price) item.price = input.sellingMode.price;
    if (input.stock) item.stock = input.stock;
    if (input.publication?.status) item.publication = { status: input.publication.status };
    item.updatedAt = new Date().toISOString();
    return json(response, 200, item, media);
  }
  if (url.pathname === '/sale/offers' && request.method === 'GET') {
    const sku = url.searchParams.get('external.id');
    const offers = Object.values(state.allegro.offers).filter((item) => !sku || item.external?.id === sku);
    return json(response, 200, { offers, count: offers.length, totalCount: offers.length }, media);
  }
  return json(response, 404, { errors: [{ message: 'Unknown Allegro path in the mock' }] }, media);
};

const hosts = {
  'shop.mock.test': shopify,
  'storage.mock.test': storage,
  'lwa.mock.test': lwa,
  'sellingpartnerapi.mock.test': spApi,
  's3.mock.test': s3,
  'allegro.mock.test': allegro,
};

const secretsSeen = [
  credentials.shopify.clientSecret,
  credentials.amazon.clientSecret,
  credentials.allegro.clientSecret,
];

const server = https.createServer(
  {
    cert: readFileSync(`${fixtures}server.pem`),
    key: readFileSync(`${fixtures}server.key`),
  },
  async (request, response) => {
    const host = (request.headers.host ?? '').split(':')[0];
    const url = new URL(request.url, `https://${host}`);
    const body = await readBody(request);
    const entry = {
      at: new Date().toISOString(),
      host,
      method: request.method,
      path: url.pathname,
      query: url.search,
      contentType: request.headers['content-type'] ?? null,
      idempotencyKey: request.headers['idempotency-key'] ?? null,
      authenticated: Boolean(
        request.headers.authorization ||
          request.headers['x-shopify-access-token'] ||
          request.headers['x-amz-access-token'],
      ),
      bytes: body.length,
    };
    state.requests.push(entry);
    const failure = injected(host, url.pathname);
    if (failure?.mode === 'hang') {
      // The destination applies the change but the answer never arrives in
      // time: the caller cannot know whether it succeeded.
      entry.status = 'hung';
      const handler = hosts[host];
      const discard = { writeHead: () => discard, end: () => {}, setHeader: () => {}, statusCode: 0 };
      await handler?.(request, discard, url, body);
      await new Promise((resolve) => setTimeout(resolve, failure.delayMs ?? 15000));
      return json(response, 504, { errors: [{ message: 'Too late' }] });
    }
    if (failure) {
      entry.status = failure.status;
      return json(response, failure.status, { errors: [{ message: 'Injected failure' }] }, failure.retryAfter ? { 'retry-after': String(failure.retryAfter) } : {});
    }
    const handler = hosts[host];
    if (!handler) return json(response, 421, { error: 'unknown mock host' });
    const finish = response.end.bind(response);
    response.end = (...args) => {
      entry.status = response.statusCode;
      return finish(...args);
    };
    try {
      await handler(request, response, url, body);
    } catch (error) {
      json(response, 500, { error: String(error) });
    }
  },
);

const summary = () => ({
  shopify: {
    products: state.shopify.products,
    staged: state.shopify.staged,
    tokensIssued: state.shopify.tokens.size,
  },
  amazon: {
    listings: state.amazon.listings,
    feeds: state.amazon.feeds,
    tokensIssued: state.amazon.tokens.size,
  },
  allegro: {
    offers: state.allegro.offers,
    pendingDevices: Object.values(state.allegro.devices).map((device) => ({ userCode: device.userCode, approved: device.approved })),
    refreshTokensUsed: [...state.allegro.refreshTokens.values()].filter(Boolean).length,
    tokensIssued: state.allegro.tokens.size,
  },
  injections: state.injections,
  leaks: state.leaks,
  requestCount: state.requests.length,
});

const page = () => `<!doctype html><html lang="en"><head><meta charset="utf-8"><title>Marketplace mock</title>
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>body{font:14px system-ui;margin:16px;color:#1d1b20;background:#fff}@media (prefers-color-scheme:dark){body{color:#e6e1e5;background:#1c1b1f}}
pre{white-space:pre-wrap;word-break:break-word}h2{margin-top:24px}</style></head><body>
<h1>Marketplace mock</h1><p>Local stand-ins for Shopify, Amazon SP-API and Allegro. Nothing here reaches a real service.</p>
<h2>State</h2><pre>${JSON.stringify(summary(), null, 2).replace(/</g, '&lt;')}</pre>
<h2>Last requests</h2><pre>${JSON.stringify(state.requests.slice(-50), null, 2).replace(/</g, '&lt;')}</pre></body></html>`;

const inspector = http.createServer(async (request, response) => {
  const url = new URL(request.url, 'http://inspect');
  const body = await readBody(request);
  if (url.pathname === '/' && request.method === 'GET') {
    response.writeHead(200, { 'content-type': 'text/html; charset=utf-8' });
    return response.end(page());
  }
  if (url.pathname === '/state') return json(response, 200, summary());
  if (url.pathname === '/requests') return json(response, 200, state.requests);
  if (url.pathname === '/reset' && request.method === 'POST') {
    state = freshState();
    return json(response, 200, { reset: true });
  }
  if (url.pathname === '/fail' && request.method === 'POST') {
    const input = JSON.parse(body.toString());
    state.injections.push({ times: 1, ...input });
    return json(response, 200, state.injections);
  }
  if (url.pathname === '/allegro/approve' && request.method === 'POST') {
    const input = JSON.parse(body.toString());
    const device = Object.values(state.allegro.devices).find((item) => item.userCode === input.user_code);
    if (!device) return json(response, 404, { error: 'unknown user code' });
    device.approved = true;
    return json(response, 200, { approved: true });
  }
  if (url.pathname === '/secrets-check') {
    // Requests never carried an operator's client secret where it does not belong.
    const leaked = state.requests.filter((entry) => secretsSeen.some((secret) => entry.query.includes(secret)));
    return json(response, 200, { leaked });
  }
  return json(response, 404, { error: 'not found' });
});

server.listen(httpsPort, '127.0.0.1', () => {
  inspector.listen(inspectPort, '127.0.0.1', () => {
    console.log(`marketplace mock: https://*.mock.test -> 127.0.0.1:${httpsPort}, inspect http://127.0.0.1:${inspectPort}/`);
  });
});

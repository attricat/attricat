#!/usr/bin/env node
// Deliberately a fixed-fixture loader, not a procedural catalog generator.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { pathToFileURL } from "node:url";

const defaultFixture = new URL("./catalog.json", import.meta.url);
const isIdentifier = (value) =>
  typeof value === "string" && /^[a-z][a-z0-9_]*$/.test(value);
const same = (actual, expected, message) =>
  assert.deepEqual(actual, expected, message);

export function validate(fixture) {
  assert.equal(fixture.format_version, 1, "unsupported fixture version");
  assert.equal(
    fixture.context,
    "default",
    "only the default context is supported",
  );
  same(
    fixture.publication_channels,
    ["default"],
    "only the default publication channel is supported",
  );
  assert.ok(Array.isArray(fixture.blueprints) && fixture.blueprints.length > 0);
  const blueprints = new Map();
  for (const bp of fixture.blueprints) {
    assert.ok(
      isIdentifier(bp.code) && !blueprints.has(bp.code),
      `invalid/duplicate blueprint ${bp.code}`,
    );
    assert.ok(typeof bp.name === "string" && bp.name.length > 0);
    const attrs = new Map();
    for (const attr of bp.attributes) {
      assert.ok(
        isIdentifier(attr.code) && !attrs.has(attr.code),
        `invalid/duplicate attribute ${attr.code}`,
      );
      assert.ok(["string", "number", "relationship"].includes(attr.value_type));
      const allowed =
        attr.value_type === "relationship"
          ? ["code", "value_type", "target_blueprint", "cardinality"]
          : ["code", "value_type", "tags"];
      assert.ok(
        Object.keys(attr).every((key) => allowed.includes(key)),
        `unsupported attribute property for ${attr.code}`,
      );
      if (attr.tags !== undefined) {
        assert.ok(
          Array.isArray(attr.tags) &&
            attr.tags.every((tag) => ["display", "searchable"].includes(tag)),
          "unsupported attribute tags",
        );
      }
      if (attr.value_type === "relationship") {
        assert.ok(
          blueprints.has(attr.target_blueprint),
          "relationship blueprints must precede their users",
        );
        assert.equal(attr.cardinality, "one");
      }
      attrs.set(attr.code, attr);
    }
    assert.equal(
      attrs.get("name")?.value_type,
      "string",
      "name must be a string attribute",
    );
    for (const column of bp.table_columns) {
      assert.ok(typeof column.label === "string");
      const [field, target, extra] = column.field.split(".");
      assert.ok(
        attrs.has(field) && !extra,
        `unknown table field ${column.field}`,
      );
      if (target)
        assert.ok(
          blueprints.get(attrs.get(field).target_blueprint)?.has(target),
        );
    }
    blueprints.set(bp.code, attrs);
  }
  assert.ok(blueprints.has(fixture.product_blueprint));
  const records = new Map();
  for (const record of fixture.records) {
    assert.ok(
      isIdentifier(record.key) && !records.has(record.key),
      `invalid/duplicate key ${record.key}`,
    );
    const attrs = blueprints.get(record.blueprint);
    assert.ok(attrs, `unknown blueprint ${record.blueprint}`);
    same(
      Object.keys(record.values).sort(),
      [...attrs.values()]
        .filter((a) => a.value_type !== "relationship")
        .map((a) => a.code)
        .sort(),
      `scalar fields for ${record.key}`,
    );
    same(
      Object.keys(record.relationships).sort(),
      [...attrs.values()]
        .filter((a) => a.value_type === "relationship")
        .map((a) => a.code)
        .sort(),
      `relationship fields for ${record.key}`,
    );
    for (const [code, value] of Object.entries(record.values)) {
      assert.equal(
        typeof value,
        attrs.get(code).value_type,
        `type of ${record.key}.${code}`,
      );
      if (typeof value === "number") assert.ok(Number.isFinite(value));
    }
    if (record.blueprint === fixture.product_blueprint) {
      assert.ok(
        typeof record.values.source_url === "string",
        `missing source URL for ${record.key}`,
      );
      assert.equal(
        new URL(record.values.source_url).protocol,
        "https:",
        "product sources must use HTTPS",
      );
    }
    records.set(record.key, record);
  }
  const products = fixture.records.filter(
    (r) => r.blueprint === fixture.product_blueprint,
  );
  assert.ok(
    products.length > 0 && products.length <= 100,
    "fixture must contain 1–100 products",
  );
  for (const record of fixture.records) {
    for (const [code, targets] of Object.entries(record.relationships)) {
      assert.ok(
        Array.isArray(targets) && targets.length === 1,
        `one target required for ${record.key}.${code}`,
      );
      assert.equal(
        records.get(targets[0])?.blueprint,
        blueprints.get(record.blueprint).get(code).target_blueprint,
        `invalid target ${targets[0]}`,
      );
    }
  }
  same(
    fixture.navigation.map((n) => n.blueprint_code).sort(),
    [...blueprints.keys()].sort(),
    "navigation must include every blueprint exactly once",
  );
  return fixture;
}

// Only the small supported schema vocabulary is rendered; all content comes from JSON.
export function definition(bp) {
  const quote = JSON.stringify;
  const fields = (object) =>
    Object.entries(object)
      .map(([key, value]) => {
        assert.ok(isIdentifier(key), `invalid TOML key ${key}`);
        return `${key} = ${quote(value)}`;
      })
      .join("\n");
  return `format_version = 1\ncode = ${quote(bp.code)}\nname = ${quote(bp.name)}\nkind = "entity"\n\n[views.dropdown_option]\ntype = "dropdown_option"\nfields = ["name"]\n\n[views.table]\ntype = "table"\ncolumns = [${bp.table_columns.map((c) => `{ field = ${quote(c.field)}, label = ${quote(c.label)} }`).join(", ")}]\n\n${bp.attributes.map((a) => `[[attributes]]\n${fields(a)}`).join("\n\n")}\n`;
}

export async function readFixture(path = defaultFixture) {
  const bytes = await readFile(path);
  const fixture = validate(JSON.parse(bytes));
  return { fixture, hash: createHash("sha256").update(bytes).digest("hex") };
}

export async function connect(
  server = process.env.CATALOG_SERVER ?? "http://api:3000",
) {
  let cookies = [];
  let csrf = "";
  async function request(path, method = "GET", body) {
    const response = await fetch(new URL(path, server), {
      method,
      headers: {
        "content-type": "application/json",
        cookie: cookies.join("; "),
        "x-catalog-csrf": csrf,
      },
      body: body === undefined ? undefined : JSON.stringify(body),
      signal: AbortSignal.timeout(30_000),
      redirect: "error",
    });
    if (!response.ok)
      throw new Error(
        `${method} ${path}: HTTP ${response.status} ${(await response.text()).slice(0, 1000)}`,
      );
    if (path === "/auth/login") {
      cookies = response.headers.getSetCookie().map((c) => c.split(";")[0]);
      csrf =
        cookies
          .find((c) => c.startsWith("catalog_csrf="))
          ?.slice("catalog_csrf=".length) ?? "";
      assert.ok(
        csrf && cookies.some((c) => c.startsWith("catalog_session=")),
        "missing login cookies",
      );
    }
    const text = await response.text();
    return text ? JSON.parse(text) : null;
  }
  const session = await request("/auth/login", "POST", {
    login_identifier: process.env.CATALOG_LOGIN_IDENTIFIER ?? "default.local",
    email: process.env.ATTRICAT_OWNER_EMAIL ?? "owner@example.com",
    password: process.env.ATTRICAT_OWNER_PASSWORD ?? "test",
  });
  return { request, session, logout: () => request("/auth/logout", "POST") };
}

async function search(request, code) {
  const items = [];
  let cursor;
  do {
    const page = await request("/v1/entities/search", "POST", {
      blueprint: { code },
      page: { size: 100, ...(cursor ? { cursor } : {}) },
    });
    items.push(...page.items);
    cursor = page.next_cursor;
  } while (cursor);
  return items;
}

export async function load(request, fixture) {
  const existing = await request("/blueprints?include_drafts=true");
  same(existing, [], "refusing to load a nonempty workspace; reset it first");
  const context = await request(`/contexts/${fixture.context}`);
  for (const bp of fixture.blueprints) {
    const created = await request("/blueprints", "POST", {
      definition: definition(bp),
    });
    await request(
      `/blueprints/${created.blueprint.id}/versions/${created.blueprint.version}/publish`,
      "POST",
    );
  }
  await request(`/publication-channels/${context.id}`, "PUT", {
    enabled: true,
  });
  const ids = new Map();
  for (const record of fixture.records) {
    const entity = await request("/v1/entities", "POST", {
      blueprint: { code: record.blueprint },
      values: Object.entries(record.values).map(([attribute_code, value]) => ({
        kind: "scalar",
        attribute_code,
        value,
        context_id: context.id,
      })),
      system_metadata: { demo_key: record.key },
    });
    ids.set(record.key, entity.id);
  }
  for (const record of fixture.records) {
    const id = ids.get(record.key);
    const relationships = Object.entries(record.relationships).map(
      ([attribute_code, targets]) => ({
        attribute_code,
        context_id: context.id,
        target_entity_ids: targets.map((key) => ids.get(key)),
      }),
    );
    if (relationships.length)
      await request(`/entities/${id}/relationships/replace`, "POST", {
        relationships,
      });
    await request(`/v1/entities/${id}/publications/publish-all`, "POST");
  }
  await request("/workspace/navigation", "PUT", {
    explore_navigation: fixture.navigation,
  });
}

export async function verify(request, fixture) {
  const context = await request(`/contexts/${fixture.context}`);
  const channels = await request("/publication-channels");
  same(
    channels
      .filter((c) => c.enabled)
      .map((c) => c.context_code)
      .sort(),
    [...fixture.publication_channels].sort(),
    "publication channels",
  );
  const contexts = await request("/contexts");
  same(
    contexts.map((c) => c.code),
    [fixture.context],
    "context inventory",
  );
  const blueprints = new Map();
  const records = [];
  const keysById = new Map();
  const listed = await request("/blueprints?include_drafts=true");
  same(
    listed.map((b) => b.code).sort(),
    fixture.blueprints.map((b) => b.code).sort(),
    "blueprint inventory",
  );
  for (const bp of fixture.blueprints) {
    const actual = await request(`/blueprints/by-code/${bp.code}`);
    assert.equal(
      actual.blueprint.definition,
      definition(bp),
      `definition ${bp.code}`,
    );
    assert.equal(actual.blueprint.status, "published");
    blueprints.set(bp.code, actual);
    for (const item of await search(request, bp.code)) {
      const entity = await request(`/entities/${item.id}`);
      const key = entity.system_metadata.demo_key;
      assert.ok(
        typeof key === "string" && ![...keysById.values()].includes(key),
        "unexpected or duplicate demo key",
      );
      keysById.set(entity.id, key);
      records.push({ id: entity.id, key, blueprint: bp.code });
    }
  }
  const actualRecords = [];
  for (const record of records) {
    const attributes = new Map(
      blueprints.get(record.blueprint).attributes.map((a) => [a.id, a]),
    );
    const values = {};
    const relationships = {};
    for (const value of await request(
      `/entities/${record.id}/values/current`,
    )) {
      assert.equal(
        value.context_id,
        context.id,
        `unexpected context for ${record.key}`,
      );
      const attribute = attributes.get(value.attribute_id);
      assert.ok(attribute, "unknown attribute");
      if (attribute.value_type === "relationship") {
        const target = keysById.get(value.relationship_target_entity_id);
        assert.ok(target, "unknown relationship target");
        (relationships[attribute.code] ??= []).push(target);
      } else {
        assert.ok(
          !Object.hasOwn(values, attribute.code),
          "duplicate scalar value",
        );
        values[attribute.code] = value.value;
      }
    }
    const publications = await request(
      `/v1/entities/${record.id}/publications`,
    );
    same(
      publications.map((p) => ({ context: p.context_code, status: p.status })),
      fixture.publication_channels.map((context) => ({
        context,
        status: "published",
      })),
      `publications for ${record.key}`,
    );
    for (const targets of Object.values(relationships)) targets.sort();
    actualRecords.push({
      key: record.key,
      blueprint: record.blueprint,
      values,
      relationships,
    });
  }
  const sortRecords = (rs) =>
    [...rs].sort((a, b) => a.key.localeCompare(b.key));
  same(
    sortRecords(actualRecords),
    sortRecords(fixture.records),
    "complete catalog differs from fixture",
  );
  const navigation = await request("/workspace/navigation");
  same(navigation, fixture.navigation, "workspace navigation");
  return actualRecords.length;
}

async function main() {
  const mode = process.argv[2] ?? "load";
  assert.ok(
    ["validate", "load", "verify"].includes(mode),
    "usage: load.mjs [validate|load|verify] [catalog.json]",
  );
  const { fixture, hash } = await readFixture(process.argv[3]);
  console.log(
    JSON.stringify({
      event: "fixture_valid",
      sha256: hash,
      records: fixture.records.length,
    }),
  );
  if (mode === "validate") return;
  const client = await connect();
  try {
    if (mode === "load") await load(client.request, fixture);
    const records = await verify(client.request, fixture);
    console.log(
      JSON.stringify({ event: "catalog_verified", sha256: hash, records }),
    );
  } finally {
    await client.logout();
  }
}

if (
  process.argv[1] &&
  import.meta.url === pathToFileURL(process.argv[1]).href
) {
  main().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}

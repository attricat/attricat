import test from "node:test";
import assert from "node:assert/strict";
import { definition, readFixture, validate } from "./load.mjs";

const { fixture, hash } = await readFixture();
const copy = () => structuredClone(fixture);

test("the checked-in catalog is valid, bounded, and deterministic", async () => {
  assert.equal(
    fixture.records.filter((r) => r.blueprint === fixture.product_blueprint)
      .length,
    20,
  );
  assert.equal((await readFixture()).hash, hash);
  for (const bp of fixture.blueprints)
    assert.equal(definition(bp), definition(structuredClone(bp)));
});

test("all products have real part numbers and manufacturer source URLs", () => {
  for (const product of fixture.records.filter(
    (r) => r.blueprint === fixture.product_blueprint,
  )) {
    assert.match(product.values.part_number, /^\d{2} \d{2} \d{3}$/);
    assert.equal(new URL(product.values.source_url).hostname, "www.knipex.com");
    assert.ok(product.values.length_mm > 0);
  }
});

for (const [name, mutate] of [
  [
    "unknown format",
    (f) => {
      f.format_version = 2;
    },
  ],
  [
    "duplicate blueprint",
    (f) => {
      f.blueprints.push(f.blueprints[0]);
    },
  ],
  [
    "missing blueprint code",
    (f) => {
      delete f.blueprints[0].code;
    },
  ],
  [
    "missing record key",
    (f) => {
      delete f.records[0].key;
    },
  ],
  [
    "unsupported attribute property",
    (f) => {
      f.blueprints[0].attributes[0].required = true;
    },
  ],
  [
    "invalid attribute tags",
    (f) => {
      f.blueprints[0].attributes[0].tags = "display";
    },
  ],
  [
    "invalid source URL",
    (f) => {
      f.records.find(
        (r) => r.blueprint === f.product_blueprint,
      ).values.source_url = "not a URL";
    },
  ],
  [
    "duplicate attribute",
    (f) => {
      f.blueprints[0].attributes.push(f.blueprints[0].attributes[0]);
    },
  ],
  [
    "duplicate record key",
    (f) => {
      f.records.push(f.records[0]);
    },
  ],
  [
    "unknown blueprint",
    (f) => {
      f.records[0].blueprint = "missing";
    },
  ],
  [
    "wrong value type",
    (f) => {
      f.records[0].values.name = 1;
    },
  ],
  [
    "missing scalar",
    (f) => {
      delete f.records[0].values.name;
    },
  ],
  [
    "unknown scalar",
    (f) => {
      f.records[0].values.extra = "oops";
    },
  ],
  [
    "dangling relationship",
    (f) => {
      f.records.find(
        (r) => r.relationships.manufacturer,
      ).relationships.manufacturer = ["missing"];
    },
  ],
  [
    "wrong relationship blueprint",
    (f) => {
      f.records.find((r) => r.relationships.category).relationships.category = [
        "knipex",
      ];
    },
  ],
  [
    "multiple targets",
    (f) => {
      f.records.find(
        (r) => r.relationships.manufacturer,
      ).relationships.manufacturer = ["knipex", "knipex"];
    },
  ],
  [
    "unknown table field",
    (f) => {
      f.blueprints[0].table_columns[0].field = "missing";
    },
  ],
  [
    "incomplete navigation",
    (f) => {
      f.navigation.pop();
    },
  ],
  [
    "more than 100 products",
    (f) => {
      const product = f.records.find(
        (r) => r.blueprint === f.product_blueprint,
      );
      for (let i = 0; i < 101; i++)
        f.records.push({ ...product, key: `extra_${i}` });
    },
  ],
]) {
  test(`rejects ${name} before any API writes`, () => {
    const changed = copy();
    mutate(changed);
    assert.throws(() => validate(changed));
  });
}

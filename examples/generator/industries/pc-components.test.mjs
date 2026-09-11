import assert from "node:assert/strict";
import test from "node:test";
import {
  blueprints,
  categoryFor,
  classificationValues,
  familyFor,
  manufacturerFor,
  profiles,
  skuFor,
} from "./pc-components.mjs";

test("profiles retain their base product-record totals", () => {
  assert.deepEqual(
    Object.fromEntries(
      Object.entries(profiles).map(([name, profile]) => [
        name,
        Object.values(profile).reduce((total, count) => total + count, 0),
      ]),
    ),
    { micro: 1000, small: 10000, medium: 100000, large: 1000000 },
  );
});

test("technical classifications are modeled as family relationships", () => {
  assert.deepEqual(
    Object.fromEntries(
      Object.entries(classificationValues).map(([kind, values]) => [
        kind,
        values.length,
      ]),
    ),
    { product_type: 8, interface_standard: 8, form_factor: 6 },
  );
  const definitions = blueprints("test");
  assert.match(
    definitions.family,
    /code = "product_type"\nvalue_type = "relationship"/,
  );
  assert.match(
    definitions.family,
    /code = "interface_standard"\nvalue_type = "relationship"/,
  );
  assert.match(
    definitions.family,
    /code = "form_factor"\nvalue_type = "relationship"/,
  );
  assert.match(
    definitions.sku,
    /code = "main_photo"\nvalue_type = "file"\nallowed_mime_groups = \["image"\]/,
  );
  assert.match(definitions.sku, /field = "family\.product_type\.name"/);
  assert.match(definitions.sku, /field = "family\.interface_standard\.name"/);
  assert.match(definitions.sku, /code = "family"[\s\S]*cardinality = "one"/);
  assert.doesNotMatch(definitions.sku, /code = "product_type"/);
  assert.doesNotMatch(definitions.sku, /code = "interface"/);
  assert.doesNotMatch(definitions.sku, /code = "form_factor"/);
});

test("generated PC catalog records are deterministic and plausible", () => {
  const family = familyFor(42, 214);
  assert.deepEqual(familyFor(42, 214), family);
  assert.match(
    family.name,
    /^(Kestrel|Meridian|Solvane|Asterion|Voltarra|Ridgepoint|Noric|Helix|Cobalt|Ternion)/,
  );
  assert.match(
    skuFor(family, 42, 1, 214).sku,
    /^BFG-[A-Z]{3}-\d{4}-\d{2}-[A-D]$/,
  );
  assert.equal(categoryFor(12).parentIndex, 4);
  assert.match(manufacturerFor(12).code, /^V\d{5}$/);
});

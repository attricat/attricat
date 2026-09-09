import assert from "node:assert/strict";
import test from "node:test";
import {
  categoryFor,
  familyFor,
  manufacturerFor,
  profiles,
  skuFor,
} from "./pc-components.mjs";

test("profiles have their documented entity totals", () => {
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

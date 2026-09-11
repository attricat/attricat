import { describe, expect, it } from 'vitest';
import { validatesJsonSchema } from './json-schema';
import { attributeSchema, blueprintSchema } from './schemas';

describe('JSON Schema validation', () => {
  it('accepts optional and nullable blueprint and attribute schemas', () => {
    expect(
      blueprintSchema.parse({
        code: 'product',
        name: 'Product',
        version: 1,
        display: {},
        entity_schema: false,
      }).entity_schema,
    ).toBe(false);
    expect(
      attributeSchema.parse({
        code: 'sku',
        value_type: 'string',
        value_schema: { minLength: 3 },
      }).value_schema,
    ).toEqual({ minLength: 3 });
    const scalarAttribute = attributeSchema.parse({
      code: 'sku',
      value_type: 'string',
      cardinality: null,
      target_cardinality: null,
    });
    expect(scalarAttribute.value_schema).toBeUndefined();
    expect(scalarAttribute.cardinality).toBeNull();
    expect(scalarAttribute.target_cardinality).toBeNull();
    expect(
      blueprintSchema.parse({
        code: 'product',
        name: 'Product',
        version: 1,
        display: {},
        entity_schema: null,
      }).entity_schema,
    ).toBeNull();
  });

  it('accepts API table views with null legacy columns', () => {
    expect(
      blueprintSchema.parse({
        code: 'product',
        name: 'Product',
        version: 1,
        views: { table: { type: 'table', fields: ['sku'], columns: null } },
      }).views.table,
    ).toMatchObject({ type: 'table', fields: ['sku'], columns: undefined });
  });

  it('validates values against schema constraints', () => {
    const schema = { type: 'string', minLength: 3 };

    expect(validatesJsonSchema('catalog', schema)).toBe(true);
    expect(validatesJsonSchema('id', schema)).toBe(false);
    expect(validatesJsonSchema('anything', true)).toBe(true);
    expect(validatesJsonSchema('anything', false)).toBe(false);
  });

  it('allows values when no schema is supplied and rejects invalid schemas', () => {
    expect(validatesJsonSchema({ any: 'value' }, undefined)).toBe(true);
    expect(validatesJsonSchema('value', { type: 'not-a-type' })).toBe(false);
  });
});

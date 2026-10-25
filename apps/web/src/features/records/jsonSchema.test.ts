import { describe, expect, it } from 'vitest';
import {
  jsonSchemaErrorMessage,
  jsonSchemaValidationErrors,
  validatesJsonSchema,
} from './jsonSchema';
import { attributeSchema, blueprintSchema } from './schemas';

const blueprintIdentity = {
  id: '123e4567-e89b-12d3-a456-426614174000',
  status: 'published' as const,
};

describe('JSON Schema validation', () => {
  it('accepts optional and nullable blueprint and attribute schemas', () => {
    expect(
      blueprintSchema.parse({
        ...blueprintIdentity,
        code: 'product',
        name: 'Product',
        version: 1,
        display: {},
        record_schema: false,
      }).record_schema,
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
        ...blueprintIdentity,
        code: 'product',
        name: 'Product',
        version: 1,
        display: {},
        record_schema: null,
      }).record_schema,
    ).toBeNull();
  });

  it('accepts API table views with null columns', () => {
    expect(
      blueprintSchema.parse({
        ...blueprintIdentity,
        code: 'product',
        name: 'Product',
        version: 1,
        views: { table: { type: 'table', columns: null } },
      }).views.table,
    ).toMatchObject({ type: 'table', columns: undefined });
  });

  it('rejects component versions outside the API u32 contract', () => {
    expect(() =>
      blueprintSchema.parse({
        ...blueprintIdentity,
        code: 'product',
        name: 'Product',
        version: 1,
        views: {
          table: {
            type: 'table',
            fields: ['sku'],
            component: { id: 'attricat.table_display', version: 4_294_967_296 },
          },
        },
      }),
    ).toThrow();
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

  it('describes the failed rule in words', () => {
    const message = (value: unknown, schema: Record<string, unknown>) =>
      jsonSchemaErrorMessage(jsonSchemaValidationErrors(value, schema) ?? []);
    expect(message('abcdef', { maxLength: 1 })).toBe(
      'Enter at most 1 character.',
    );
    expect(message(11, { maximum: 10 })).toBe('Enter 10 or less.');
    expect(message(10, { exclusiveMaximum: 10 })).toBe(
      'Enter a number less than 10.',
    );
    expect(message('x', { enum: ['a', 'b'] })).toBe(
      'Choose one of the allowed values.',
    );
    expect(message('b', { const: 'a' })).toBe('The only allowed value is a.');
    expect(message('nope', { format: 'email' })).toBe(
      'Enter an email address.',
    );
    expect(message('x', { pattern: '^a$' })).toBe(
      'Does not match the required format.',
    );
    // The hint of the subschema that failed, even inside a composite.
    expect(
      message('x', {
        anyOf: [
          { pattern: '^a$', 'x-attricat-pattern-hint': 'The letter a' },
          { pattern: '^b$', 'x-attricat-pattern-hint': 'The letter b' },
        ],
      }),
    ).toBe('Does not match the required format: The letter a');
    expect(message('x', { not: { const: 'x' } })).toBe(undefined);
  });
});

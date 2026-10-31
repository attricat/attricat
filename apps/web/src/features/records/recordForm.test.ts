import { describe, expect, it } from 'vitest';
import {
  formRecordSchema,
  missingRequiredAttributes,
  relationshipTargetsForForm,
  serializeAttributeValues,
  validateRecordForm,
  valuesForForm,
  recordFormStatus,
  recordFormStatusKinds,
} from './recordForm';
import type { Attribute } from './api';
import { urlEditComponent } from '../views/controls/definitions';

const attributes = [
  { code: 'title', value_type: 'string' },
  { code: 'related_products', value_type: 'relationship' },
] satisfies Attribute[];
const recordIdOne = '123e4567-e89b-12d3-a456-426614174000';
const recordIdTwo = '123e4567-e89b-12d3-a456-426614174001';

const typedAttributes = [
  { code: 'price', value_type: 'number' },
  { code: 'stock', value_type: 'integer' },
  { code: 'available', value_type: 'boolean' },
  { code: 'cutoff', value_type: 'time' },
] as const satisfies readonly Attribute[];

describe('record form values', () => {
  it('lists required attributes without a value, ignoring files and system fields', () => {
    const required = [
      ...attributes,
      { code: 'photo', value_type: 'file' },
      { code: 'sku', value_type: 'string', readonly: true },
    ] satisfies Attribute[];
    const codes = new Set(['title', 'related_products', 'photo', 'sku']);
    const missing = (fields: Record<string, string>) =>
      missingRequiredAttributes(required, fields, codes).map(
        (attribute) => attribute.code,
      );
    expect(missing({ title: '  ' })).toEqual(['title', 'related_products']);
    expect(missing({ title: 'Lamp', related_products: recordIdOne })).toEqual(
      [],
    );
  });

  it('treats a required file attribute as filled only when files are queued', () => {
    const withFiles = [
      { code: 'title', value_type: 'string' },
      { code: 'photo', value_type: 'file' },
    ] satisfies Attribute[];
    const codes = new Set(['title', 'photo']);
    const missing = (counts: Record<string, number>) =>
      missingRequiredAttributes(
        withFiles,
        { title: 'Lamp' },
        codes,
        counts,
      ).map((attribute) => attribute.code);
    expect(missing({})).toEqual(['photo']);
    expect(missing({ photo: 0 })).toEqual(['photo']);
    expect(missing({ photo: 2 })).toEqual([]);
  });

  it('checks the record schema against queued files', () => {
    const withFiles = [
      { code: 'title', value_type: 'string' },
      { code: 'photo', value_type: 'file' },
    ] satisfies Attribute[];
    const schema = {
      type: 'object',
      required: ['photo'],
      properties: { photo: { type: 'array', minItems: 1 } },
    };
    const validate = (
      counts?: Record<string, number>,
      required: string[] = [],
    ) =>
      validateRecordForm(
        withFiles,
        { title: 'Lamp' },
        required,
        schema,
        undefined,
        undefined,
        counts,
      );
    // Without queued files the schema requires the file attribute.
    expect(validate({}).fieldErrors.photo).toBeDefined();
    expect(validate({}, ['photo']).fieldErrors.photo).toBeDefined();
    expect(validate({ photo: 1 }, ['photo'])).toEqual({ fieldErrors: {} });
  });

  it('counts zero, false and empty JSON as values but not blank text', () => {
    const typed = [
      ...typedAttributes,
      { code: 'specs', value_type: 'json' },
      ...attributes,
    ] satisfies Attribute[];
    const codes = new Set(typed.map((attribute) => attribute.code));
    const missing = (fields: Record<string, string>) =>
      missingRequiredAttributes(typed, fields, codes).map(
        (attribute) => attribute.code,
      );
    expect(
      missing({
        price: '0',
        stock: '0',
        available: 'false',
        cutoff: '09:00 UTC',
        specs: '[]',
        title: '\t\n ',
        related_products: ' , ',
      }),
    ).toEqual(['title', 'related_products']);
    expect(
      validateRecordForm(typed, { price: '0' }, ['price', 'available'])
        .fieldErrors,
    ).toEqual({ available: expect.any(String) });
  });

  it('validates only configured URL editors, including required and optional clearing', () => {
    const validate = (title: string, required: string[] = []) =>
      validateRecordForm(
        attributes,
        { title },
        required,
        undefined,
        undefined,
        new Map([['title', urlEditComponent]]),
      );
    expect(validate('javascript:alert(1)').fieldErrors.title).toBeTruthy();
    expect(validate('https://example.com/a?q=1').fieldErrors).toEqual({});
    expect(validate('').fieldErrors).toEqual({});
    expect(validate('', ['title']).fieldErrors.title).toBeTruthy();
    expect(
      validateRecordForm(attributes, { title: 'ordinary text' }).fieldErrors,
    ).toEqual({});
    expect(
      serializeAttributeValues(attributes, {
        title: 'https://example.com/a?q=1',
      })[0],
    ).toMatchObject({ value: 'https://example.com/a?q=1' });
  });

  it('validates email value schemas while allowing optional empty values', () => {
    const attributes = [
      {
        code: 'email',
        value_type: 'string' as const,
        value_schema: { type: 'string', format: 'email' },
      },
    ];
    expect(
      validateRecordForm(attributes, { email: 'invalid' }).fieldErrors.email,
    ).toBeTruthy();
    expect(validateRecordForm(attributes, { email: '' }).fieldErrors).toEqual(
      {},
    );
    expect(
      validateRecordForm(attributes, { email: ' Name+tag@Example.com ' })
        .fieldErrors,
    ).toEqual({});
    expect(
      serializeAttributeValues(attributes, {
        email: ' Name+tag@Example.com ',
      })[0],
    ).toMatchObject({ value: 'Name+tag@Example.com' });
  });

  it('serializes scalar fields and complete relationship target sets', () => {
    expect(
      serializeAttributeValues(attributes, {
        title: 'Summer shirt',
        related_products: `${recordIdOne}, ${recordIdTwo} , `,
      }),
    ).toEqual([
      {
        kind: 'scalar',
        attribute_code: 'title',
        context_id: null,
        value: 'Summer shirt',
      },
    ]);
    expect(
      relationshipTargetsForForm(attributes, {
        related_products: `${recordIdOne}, ${recordIdTwo} , `,
      }),
    ).toEqual([
      {
        attribute_code: 'related_products',
        context_id: null,
        target_record_ids: [recordIdOne, recordIdTwo],
      },
    ]);
  });

  it('rejects malformed relationship IDs', () => {
    expect(
      relationshipTargetsForForm(attributes, {
        related_products: 'not-a-uuid',
      }),
    ).toEqual([]);
  });

  it('reports malformed relationship IDs instead of allowing omission', () => {
    expect(
      validateRecordForm(attributes, {
        related_products: `${recordIdOne}, not-a-uuid`,
      }),
    ).toEqual({
      fieldErrors: {
        related_products: 'Enter comma-separated record UUIDs.',
      },
    });
  });

  it('allows empty optional relationships and trims valid UUID lists', () => {
    expect(
      validateRecordForm(attributes, {
        related_products: ` ${recordIdOne}, ${recordIdTwo}, `,
      }),
    ).toEqual({ fieldErrors: {} });
    expect(validateRecordForm(attributes, {})).toEqual({ fieldErrors: {} });
  });

  it('hydrates relationship values into a comma-separated field', () => {
    expect(
      valuesForForm(attributes, [
        { kind: 'scalar', attribute_code: 'title', value: 'Summer shirt' },
        {
          kind: 'relationship',
          attribute_code: 'related_products',
          target_record_id: 'uuid-one',
        },
        {
          kind: 'relationship',
          attribute_code: 'related_products',
          target_record_id: 'uuid-two',
        },
      ]),
    ).toEqual({
      title: 'Summer shirt',
      related_products: 'uuid-one, uuid-two',
    });
  });

  it('serializes typed scalar values without coercing false to an omission', () => {
    expect(
      serializeAttributeValues(typedAttributes, {
        price: '49.95',
        stock: '24',
        available: 'false',
        cutoff: '09:30:00 America/New_York',
      }),
    ).toEqual([
      {
        kind: 'scalar',
        attribute_code: 'price',
        context_id: null,
        value: 49.95,
      },
      { kind: 'scalar', attribute_code: 'stock', context_id: null, value: 24 },
      {
        kind: 'scalar',
        attribute_code: 'available',
        context_id: null,
        value: false,
      },
      {
        kind: 'scalar',
        attribute_code: 'cutoff',
        context_id: null,
        value: { time: '09:30:00', time_zone: 'America/New_York' },
      },
    ]);
  });

  it('maps record schema property failures to their fields', () => {
    expect(
      validateRecordForm(
        [{ code: 'price', value_type: 'number' }],
        { price: '2' },
        [],
        {
          type: 'object',
          properties: { price: { minimum: 3 } },
        },
      ),
    ).toEqual({
      fieldErrors: { price: 'Enter 3 or more.' },
    });
  });

  it('maps required record schema properties to their fields', () => {
    expect(
      validateRecordForm([{ code: 'title', value_type: 'string' }], {}, [], {
        type: 'object',
        required: ['title'],
      }),
    ).toEqual({
      fieldErrors: { title: 'A value is required for the target schema.' },
    });
  });

  it('names a required attribute the form cannot set', () => {
    expect(
      validateRecordForm([{ code: 'title', value_type: 'string' }], {}, [], {
        type: 'object',
        required: ['product_code'],
      }),
    ).toEqual({
      fieldErrors: {},
      formError:
        "The required attribute “product code” can't be set in this form. Ask an administrator to give it a value or change the blueprint.",
    });
  });

  it('names the value schema rule a field fails', () => {
    const sku = {
      code: 'sku',
      value_type: 'string' as const,
      value_schema: {
        type: 'string',
        minLength: 3,
        pattern: '^[A-Z]{3}-[0-9]{4}$',
        'x-attricat-pattern-hint':
          'Three capital letters, a hyphen and four digits',
      },
    };
    expect(validateRecordForm([sku], { sku: 'AB' }).fieldErrors.sku).toBe(
      'Enter at least 3 characters.',
    );
    expect(validateRecordForm([sku], { sku: 'abc-12' }).fieldErrors.sku).toBe(
      'Does not match the required format: Three capital letters, a hyphen and four digits',
    );
    const quantity = {
      code: 'quantity',
      value_type: 'integer' as const,
      value_schema: { type: 'integer', exclusiveMinimum: 0, multipleOf: 5 },
    };
    expect(
      validateRecordForm([quantity], { quantity: '0' }).fieldErrors.quantity,
    ).toBe('Enter a number greater than 0.');
    expect(
      validateRecordForm([quantity], { quantity: '7' }).fieldErrors.quantity,
    ).toBe('Enter a multiple of 5.');
    // A value that is not of the type at all says so.
    expect(
      validateRecordForm([quantity], { quantity: 'many' }).fieldErrors.quantity,
    ).toBe("Enter a value that meets this field's requirements.");
  });
});

describe('formRecordSchema', () => {
  const generatedAttributes = [
    { code: 'title', value_type: 'string' },
    {
      code: 'number',
      value_type: 'string',
      readonly: true,
      generated: {
        format: 'INV-{n:04}',
        start: 1,
        reset: 'never',
        time_zone: 'UTC',
      },
    },
  ] satisfies Attribute[];

  it('leaves generated attributes to the server', () => {
    const schema = { type: 'object', required: ['number', 'title'] };
    expect(formRecordSchema(schema, generatedAttributes)).toEqual({
      type: 'object',
      required: ['title'],
    });
    expect(
      validateRecordForm(
        generatedAttributes.slice(0, 1),
        { title: 'Invoice' },
        [],
        formRecordSchema(schema, generatedAttributes),
      ),
    ).toEqual({ fieldErrors: {} });
  });

  it('keeps schemas without generated attributes unchanged', () => {
    const schema = { type: 'object', required: ['title'] };
    expect(formRecordSchema(schema, attributes)).toBe(schema);
    expect(formRecordSchema(true, generatedAttributes)).toBe(true);
    expect(formRecordSchema(undefined, generatedAttributes)).toBeUndefined();
  });
});

describe('recordFormStatus', () => {
  const base = {
    errorIds: [],
    fieldOrder: ['title', 'sku', 'price'],
    missingRequiredCodes: [],
    pendingConfirmationIds: [],
    submitAttempted: false,
  };

  it('reports errors only once a save was attempted', () => {
    const status = {
      ...base,
      errorIds: ['price'],
      missingRequiredCodes: ['sku'],
    };
    expect(recordFormStatus(status)).toEqual({
      kind: recordFormStatusKinds.required,
      count: 1,
      target: 'sku',
    });
    expect(recordFormStatus({ ...status, submitAttempted: true })).toEqual({
      kind: recordFormStatusKinds.errors,
      count: 1,
      target: 'price',
    });
  });

  it('goes to the first missing field in form order', () => {
    expect(
      recordFormStatus({ ...base, missingRequiredCodes: ['price', 'title'] }),
    ).toEqual({
      kind: recordFormStatusKinds.required,
      count: 2,
      target: 'title',
    });
  });

  it('reports confirmations last, and nothing when nothing blocks a save', () => {
    expect(
      recordFormStatus({ ...base, pendingConfirmationIds: ['status-change'] }),
    ).toEqual({
      kind: recordFormStatusKinds.confirm,
      count: 1,
      target: 'status-change',
    });
    expect(recordFormStatus(base)).toBeUndefined();
  });
});

import { describe, expect, it } from 'vitest';
import type { Rule } from '../rules/schemas';
import type { Attribute } from './api';
import {
  attributeConnectionKinds,
  blueprintConnections,
  predicateAttributeCodes,
} from './attributeConnections';

const saleStatus: Attribute = {
  code: 'sale_status',
  name: 'Sale status',
  value_type: 'string',
  value_schema: {
    type: 'string',
    enum: ['available', 'reserved', 'sold'],
    'x-attricat-status': {
      version: 1,
      options: [
        { code: 'available', label: 'Available' },
        { code: 'reserved', label: 'Reserved', lock: ['price'] },
        { code: 'sold', label: 'Sold', lock: 'all' },
      ],
      transitions: [
        { from: null, to: 'available' },
        {
          from: 'available',
          to: 'reserved',
          conditions: [
            {
              code: 'has-date',
              predicate: { type: 'required', attribute_code: 'reserved_until' },
            },
          ],
        },
      ],
    },
  },
};
const attributes: Attribute[] = [
  saleStatus,
  { code: 'price', value_type: 'number' },
  { code: 'reserved_until', value_type: 'date' },
];

describe('predicateAttributeCodes', () => {
  it('reads only this record’s attributes inside linked predicates', () => {
    expect(
      predicateAttributeCodes({
        type: 'all_of',
        predicates: [
          {
            type: 'compare',
            attribute_code: 'sale_price',
            op: 'lt',
            other_attribute_code: 'price',
          },
          {
            type: 'linked',
            relationship_code: 'model',
            predicate: {
              type: 'compare',
              attribute_code: 'body_types',
              op: 'eq',
              subject_attribute_code: 'body_type',
            },
          },
          {
            type: 'referenced_by',
            blueprint_code: 'order',
            relationship_code: 'vehicle',
          },
        ],
      }),
    ).toEqual(['sale_price', 'price', 'model', 'body_type']);
  });
});

describe('blueprintConnections', () => {
  it('finds requirements, conditions and checks in the record schema', () => {
    const connections = blueprintConnections({
      attributes: [],
      recordSchema: {
        required: ['title'],
        dependentRequired: { on_sale: ['sale_price'] },
        allOf: [
          {
            if: { properties: { condition: { const: 'used' } } },
            then: { required: ['mileage_km'] },
          },
        ],
        'x-attricat-checks': [
          {
            code: 'below-list',
            message: 'The sale price must be lower than the list price',
            predicate: {
              type: 'compare',
              attribute_code: 'sale_price',
              op: 'lt',
              other_attribute_code: 'price',
            },
          },
        ],
      },
      definition: undefined,
    });

    expect(connections).toEqual([
      { kind: attributeConnectionKinds.required, attributeCodes: ['title'] },
      {
        kind: attributeConnectionKinds.schemaCondition,
        attributeCodes: ['on_sale', 'sale_price'],
        sources: ['on_sale'],
      },
      {
        kind: attributeConnectionKinds.schemaCondition,
        attributeCodes: ['condition', 'mileage_km'],
        sources: ['condition'],
      },
      {
        kind: attributeConnectionKinds.check,
        attributeCodes: ['sale_price', 'price'],
        name: 'The sale price must be lower than the list price',
      },
    ]);
  });

  it('groups status locks by option and finds transition conditions', () => {
    const connections = blueprintConnections({
      attributes,
      recordSchema: null,
      definition: undefined,
    });

    expect(connections).toEqual([
      {
        kind: attributeConnectionKinds.statusLock,
        attributeCodes: ['sale_status', 'price'],
        sources: ['sale_status'],
        name: 'Sale status',
        detail: 'Reserved, Sold',
      },
      {
        kind: attributeConnectionKinds.statusLock,
        attributeCodes: ['sale_status', 'reserved_until'],
        sources: ['sale_status'],
        name: 'Sale status',
        detail: 'Sold',
      },
      {
        kind: attributeConnectionKinds.transitionCondition,
        attributeCodes: ['sale_status', 'reserved_until'],
        sources: ['sale_status'],
        name: 'Sale status',
        detail: 'Available → Reserved',
      },
    ]);
  });

  it('reads unique keys, the dropdown label and includes from the definition', () => {
    const connections = blueprintConnections({
      attributes: [],
      recordSchema: null,
      definition: `
attributes:
  - code: meta_description
    from: seo.meta_description
includes:
  - alias: seo
    code: cars_seo
    version: 1
unique_keys:
  - code: vin
    attributes: [vin]
views:
  dropdown_option:
    type: dropdown_option
    fields: [stock_number, title]
`,
    });

    expect(connections).toEqual([
      {
        kind: attributeConnectionKinds.uniqueKey,
        attributeCodes: ['vin'],
        name: 'vin',
      },
      {
        kind: attributeConnectionKinds.dropdownLabel,
        attributeCodes: ['stock_number', 'title'],
      },
      {
        kind: attributeConnectionKinds.include,
        attributeCodes: ['meta_description'],
        name: 'cars_seo',
        version: 1,
        detail: 'seo.meta_description',
      },
    ]);
  });

  it('ignores a definition it cannot parse', () => {
    expect(
      blueprintConnections({
        attributes: [],
        recordSchema: null,
        definition: '[[',
      }),
    ).toEqual([]);
  });

  it('includes the attributes rules check and enforce', () => {
    const rule = {
      name: 'Sold vehicles record the sale date',
      compiled_plan: {
        predicate: { type: 'required', attribute_code: 'sold_on' },
        enforcement: {
          transitions: [{ attribute_code: 'sale_status', to: 'sold' }],
        },
      },
    } as Rule;

    expect(
      blueprintConnections({
        attributes: [],
        recordSchema: null,
        definition: undefined,
        rules: [rule],
      }),
    ).toEqual([
      {
        kind: attributeConnectionKinds.rule,
        attributeCodes: ['sold_on', 'sale_status'],
        name: 'Sold vehicles record the sale date',
      },
    ]);
  });
});

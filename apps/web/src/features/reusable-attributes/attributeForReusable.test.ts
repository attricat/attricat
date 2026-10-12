import { describe, expect, it } from 'vitest';
import type { ReusableAttribute } from './api';
import { attributeForReusable } from './attributeForReusable';

const reusable: ReusableAttribute = {
  cardinality: null,
  code: 'size',
  context_editable: 'default',
  context_fallback: 'none',
  default_value: 'small',
  definition: '',
  definition_id: '123e4567-e89b-12d3-a456-426614174000',
  facetable: true,
  file_policy: null,
  id: '123e4567-e89b-12d3-a456-426614174001',
  name: 'Size',
  namespace: 'acme',
  published_at: null,
  readonly: false,
  searchable: false,
  status: 'draft',
  tags: ['hidden:explorer'],
  target_blueprint_code: null,
  target_cardinality: null,
  value_schema: { enum: ['small', 'large'], type: 'string' },
  value_type: 'string',
  version: 1,
};

describe('attributeForReusable', () => {
  it('keeps what the record form needs to edit and check a value', () => {
    expect(attributeForReusable(reusable)).toMatchObject({
      code: 'size',
      context_editable: 'default',
      context_fallback: 'none',
      name: 'Size',
      tags: ['hidden:explorer'],
      value_schema: { enum: ['small', 'large'], type: 'string' },
      value_type: 'string',
    });
  });
});

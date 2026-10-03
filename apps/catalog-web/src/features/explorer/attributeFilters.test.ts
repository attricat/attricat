import i18n from 'i18next';
import { describe, expect, it } from 'vitest';
import '../../i18n';
import type { Attribute } from '../entities/api';
import {
  attributeFilterLabel,
  operatorsForAttribute,
} from './attributeFilters';

const statusAttribute: Attribute = {
  code: 'status',
  value_type: 'string',
  value_schema: {
    type: 'string',
    enum: ['draft', 'live'],
    'x-attricat-status': {
      version: 1,
      options: [
        { code: 'draft', label: 'Draft' },
        { code: 'live', label: 'Live' },
      ],
    },
  },
};

describe('attribute filters', () => {
  it('match status codes exactly and show their labels', () => {
    expect(operatorsForAttribute(statusAttribute)).toEqual(['eq']);
    expect(
      operatorsForAttribute({ code: 'title', value_type: 'string' }),
    ).toEqual(['eq', 'contains', 'starts_with']);
    const filter = { field: 'status', operator: 'eq' as const, value: 'live' };
    expect(
      attributeFilterLabel(i18n.t, filter, 'Status', statusAttribute),
    ).toContain('"Live"');
    expect(attributeFilterLabel(i18n.t, filter, 'Status')).toContain('"live"');
  });
});

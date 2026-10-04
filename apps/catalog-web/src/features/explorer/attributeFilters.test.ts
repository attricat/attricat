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

const assigneeAttribute: Attribute = {
  code: 'assignee',
  value_type: 'string',
  value_schema: {
    type: 'string',
    'x-attricat-principal': { version: 1, kinds: ['user', 'team'] },
  },
};

describe('attribute filters', () => {
  it('match assignments exactly and show names or "assigned to me"', () => {
    expect(operatorsForAttribute(assigneeAttribute)).toEqual(['eq']);
    const teamId = '8c3f9a54-2d0c-4f3a-9a7e-1c2b3d4e5f62';
    const directory = {
      users: [],
      teams: [{ id: teamId, code: 'qa', name: 'Quality', deleted: false }],
    };
    expect(
      attributeFilterLabel(
        i18n.t,
        { field: 'assignee', operator: 'eq', value: `team:${teamId}` },
        assigneeAttribute,
        directory,
      ),
    ).toContain('"Quality"');
    expect(
      attributeFilterLabel(
        i18n.t,
        { field: 'assignee', operator: 'eq', value: '@me' },
        assigneeAttribute,
      ),
    ).toContain(i18n.t('explorer.assignedToMe'));
  });

  it('match status codes exactly and show their labels', () => {
    expect(operatorsForAttribute(statusAttribute)).toEqual(['eq']);
    expect(
      operatorsForAttribute({ code: 'title', value_type: 'string' }),
    ).toEqual(['eq', 'contains', 'starts_with']);
    const filter = { field: 'status', operator: 'eq' as const, value: 'live' };
    expect(attributeFilterLabel(i18n.t, filter, statusAttribute)).toContain(
      '"Live"',
    );
    expect(attributeFilterLabel(i18n.t, filter)).toContain('"live"');
  });
});

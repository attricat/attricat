import { describe, expect, it } from 'vitest';
import {
  statusConfiguration,
  statusLabel,
  statusParentContexts,
  statusTransitionAllowed,
  savedStatusValue,
} from './status';
import type { Attribute } from './api';

export const statusAttribute: Attribute = {
  code: 'status',
  value_type: 'string',
  value_schema: {
    type: 'string',
    enum: ['draft', 'live', 'done'],
    'x-attricat-status': {
      version: 1,
      options: [
        { code: 'draft', label: 'Draft' },
        { code: 'live', label: 'Live', tone: 'success' },
        { code: 'done', label: 'Done' },
      ],
      transitions: [
        { from: null, to: 'draft' },
        { from: 'draft', to: 'live' },
        { from: 'live', to: 'done' },
      ],
    },
  },
};

describe('status configuration', () => {
  it('uses stable codes and display labels', () => {
    expect(statusLabel(statusAttribute, 'live')).toBe('Live');
    expect(statusLabel(statusAttribute, 'retired')).toBeUndefined();
    expect(
      statusConfiguration({ code: 'text', value_type: 'string' }),
    ).toBeUndefined();
  });
  it('checks initial, terminal, unchanged and forbidden transitions', () => {
    const config = statusConfiguration(statusAttribute)!;
    expect(statusTransitionAllowed(config, null, 'draft')).toBe(true);
    expect(statusTransitionAllowed(config, null, 'live')).toBe(false);
    expect(statusTransitionAllowed(config, 'draft', 'live')).toBe(true);
    expect(statusTransitionAllowed(config, 'draft', 'done')).toBe(false);
    expect(statusTransitionAllowed(config, 'done', 'done')).toBe(true);
    expect(statusTransitionAllowed(config, 'done', 'live')).toBe(false);
    expect(statusTransitionAllowed(config, 'live', null)).toBe(false);
  });
  it('resolves the saved inherited baseline and guards cyclic context paths', () => {
    const path = statusParentContexts(
      [
        { id: 'child', parent_id: 'default' },
        { id: 'default', parent_id: 'child' },
      ],
      'child',
    );
    expect(path).toEqual(['default']);
    expect(
      savedStatusValue(
        statusAttribute,
        [
          {
            kind: 'scalar',
            attribute_code: 'status',
            context_id: 'default',
            value: 'live',
          },
        ],
        ['child', ...path],
      ),
    ).toBe('live');
  });
});

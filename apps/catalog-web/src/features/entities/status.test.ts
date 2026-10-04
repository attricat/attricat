import { describe, expect, it } from 'vitest';
import {
  statusConfiguration,
  statusLabel,
  statusLocks,
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

describe('status locks', () => {
  const controlled: Attribute = {
    ...statusAttribute,
    value_schema: {
      type: 'string',
      enum: ['draft', 'live', 'done'],
      'x-attricat-status': {
        version: 1,
        options: [
          { code: 'draft', label: 'Draft' },
          { code: 'live', label: 'Live', lock: ['title'] },
          { code: 'done', label: 'Done', lock: 'all' },
        ],
        transitions: [
          { from: null, to: 'draft' },
          { from: 'draft', to: 'live', code: 'release', roles: ['reviewer'] },
          { from: 'live', to: 'done' },
        ],
      },
    },
  };
  const attributes: Attribute[] = [
    controlled,
    { code: 'title', value_type: 'string' },
    { code: 'notes', value_type: 'string' },
  ];
  const saved = (status: string) => [
    {
      kind: 'scalar' as const,
      attribute_code: 'status',
      context_id: 'default',
      value: status,
    },
  ];

  it('follows the saved status of the context', () => {
    expect(statusLocks(attributes, saved('draft'), 'default', [])).toEqual({});
    expect(statusLocks(attributes, saved('live'), 'default', [])).toEqual({
      title: 'Live',
    });
    expect(statusLocks(attributes, saved('done'), 'default', [])).toEqual({
      title: 'Done',
      notes: 'Done',
    });
    expect(statusLocks(attributes, saved('done'), 'web', ['default'])).toEqual({
      title: 'Done',
      notes: 'Done',
    });
  });
});

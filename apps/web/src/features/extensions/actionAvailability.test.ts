import { describe, expect, it } from 'vitest';
import { actionAvailability } from './actionAvailability';
import type { ActionTarget } from './actions';

const target: ActionTarget = {
  blueprintId: 'b',
  blueprintVersion: 1,
  blueprintCode: 'product',
  contextId: null,
  recordIds: ['r1', 'r2'],
  canWrite: true,
  recordTags: [['checked'], ['checked', 'archived']],
};
const action = (when: Parameters<typeof actionAvailability>[0]['when']) => ({
  extension_id: 'acme',
  id: 'go',
  when,
});

describe('actionAvailability', () => {
  it('offers an action whose conditions hold for every record', () => {
    expect(
      actionAvailability(
        action({
          blueprints: ['product'],
          min_records: 1,
          has_tags: ['checked'],
        }),
        target,
      ),
    ).toEqual({ offered: true });
  });

  it('hides an action whose conditions fail', () => {
    for (const when of [
      { blueprints: ['vehicle'] },
      { min_records: 3 },
      { max_records: 1 },
      { has_tags: ['archived'] },
      { missing_tags: ['archived'] },
    ])
      expect(actionAvailability(action(when), target).offered).toBe(false);
    expect(
      actionAvailability(action({ requires_write: true }), {
        ...target,
        canWrite: false,
      }).offered,
    ).toBe(false);
  });

  it('keeps an action the surface cannot judge', () => {
    expect(
      actionAvailability(action({ requires_write: true, has_tags: ['x'] }), {
        ...target,
        blueprintCode: undefined,
        canWrite: undefined,
        recordTags: undefined,
      }).offered,
    ).toBe(true);
  });

  it('applies the state its extension gave for the record', () => {
    const states = new Map([['acme:go', { disabled: 'Select a context' }]]);
    expect(actionAvailability(action(null), target, states)).toEqual({
      offered: true,
      disabledReason: 'Select a context',
    });
    expect(
      actionAvailability(
        action(null),
        target,
        new Map([['acme:go', { hidden: true as const }]]),
      ).offered,
    ).toBe(false);
  });
});

import { describe, expect, it } from 'vitest';
import { reduceToasts, type Toast } from './toast-queue';

const makeToast = (overrides: Partial<Toast> = {}): Toast => ({
  count: 1,
  id: 1,
  message: 'Saved',
  severity: 'success',
  ...overrides,
});

describe('reduceToasts', () => {
  it('queues distinct notifications in order', () => {
    const first = makeToast();
    const second = makeToast({ id: 2, message: 'Published' });

    const queue = reduceToasts(
      reduceToasts([], { toast: first, type: 'show' }),
      { toast: second, type: 'show' },
    );

    expect(queue).toEqual([
      { ...first, dedupeKey: 'success:Saved' },
      { ...second, dedupeKey: 'success:Published' },
    ]);
  });

  it('deduplicates matching notifications already in the queue', () => {
    const queue = reduceToasts(
      reduceToasts([], { toast: makeToast(), type: 'show' }),
      { toast: makeToast({ autoHideDuration: 1000, id: 2 }), type: 'show' },
    );

    expect(queue).toEqual([
      {
        autoHideDuration: 1000,
        count: 2,
        dedupeKey: 'success:Saved',
        id: 1,
        message: 'Saved',
        severity: 'success',
      },
    ]);
  });

  it('counts notifications in a custom group', () => {
    const queue = reduceToasts(
      reduceToasts([], {
        toast: makeToast({ dedupeKey: 'entity-save', message: 'Saving…' }),
        type: 'show',
      }),
      {
        toast: makeToast({
          dedupeKey: 'entity-save',
          id: 2,
          message: 'Saved',
        }),
        type: 'show',
      },
    );

    expect(queue).toEqual([
      {
        count: 2,
        dedupeKey: 'entity-save',
        id: 1,
        message: 'Saved',
        severity: 'success',
      },
    ]);
  });
});

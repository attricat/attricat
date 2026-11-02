import {
  MUTATION_TYPE_INSERT_CHILD,
  MUTATION_TYPE_REMOVE_CHILD,
  MUTATION_TYPE_UPDATE_PROPERTY,
  NODE_TYPE_ELEMENT,
  ROOT_ID,
} from '@remote-dom/core';
import { describe, expect, it, vi } from 'vitest';
import { NativeReceiver } from './receiver';
import { uiKitVersions } from './generated/uiKit';

const textField = (id: string) => ({
  id,
  type: NODE_TYPE_ELEMENT,
  element: 'attricat-text-field',
  properties: { label: 'Name' },
  children: [],
});

describe('NativeReceiver', () => {
  it('keeps subscriptions and methods of an element moved within a batch', () => {
    const receiver = new NativeReceiver(
      uiKitVersions[uiKitVersions.length - 1],
      {
        countNodes: vi.fn(),
        countMutations: vi.fn(),
        warn: vi.fn(),
      },
    );
    receiver.mutate([
      [MUTATION_TYPE_INSERT_CHILD, ROOT_ID, textField('a'), 0],
      [MUTATION_TYPE_INSERT_CHILD, ROOT_ID, textField('b'), 1],
    ] as never);
    const changed = vi.fn();
    const focus = vi.fn();
    receiver.subscribe('a', changed);
    receiver.registerMethods('a', { focus });
    // The SDK sends a move as a removal and an insertion under the same id.
    receiver.mutate([
      [MUTATION_TYPE_REMOVE_CHILD, ROOT_ID, 0],
      [MUTATION_TYPE_INSERT_CHILD, ROOT_ID, textField('a'), 1],
    ] as never);
    changed.mockClear();
    receiver.mutate([
      [MUTATION_TYPE_UPDATE_PROPERTY, 'a', 'label', 'Title'],
    ] as never);
    expect(changed).toHaveBeenCalled();
    receiver.call('a', 'focus');
    expect(focus).toHaveBeenCalled();
  });
});

// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { EntityItem } from '../entities/api';
import { useExplorerSelection } from './useExplorerSelection';

const first = { id: 'first' } as EntityItem;
const second = { id: 'second' } as EntityItem;

describe('useExplorerSelection', () => {
  it('keeps selection when loaded results change and clears it on exit', () => {
    const { result, rerender } = renderHook(
      ({ items }) => useExplorerSelection('product', items),
      { initialProps: { items: [first, second] } },
    );
    act(() => result.current.toggleSelectionMode());
    act(() => result.current.toggleLoaded());
    expect(result.current.selectedItems).toEqual([first, second]);

    rerender({ items: [second] });
    expect(result.current.selectedItems).toEqual([first, second]);
    expect(result.current.allLoadedSelected).toBe(true);

    act(() => result.current.toggleLoaded());
    expect(result.current.selectedItems).toEqual([first]);

    act(() => result.current.exitSelectionMode());
    expect(result.current.selectionMode).toBe(false);
    expect(result.current.selectedItems).toEqual([]);
  });

  it('starts afresh for another blueprint', () => {
    const { result, rerender } = renderHook(
      ({ scope }) => useExplorerSelection(scope, [first]),
      { initialProps: { scope: 'product' } },
    );
    act(() => result.current.toggleSelectionMode());
    act(() => result.current.toggleEntity(first));
    rerender({ scope: 'category' });
    expect(result.current.selectionMode).toBe(false);
    expect(result.current.selectedItems).toEqual([]);
    rerender({ scope: 'product' });
    expect(result.current.selectedItems).toEqual([]);
  });
});

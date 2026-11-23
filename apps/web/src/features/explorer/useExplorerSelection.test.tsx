// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { RecordItem } from '../records/api';
import { useExplorerSelection } from './useExplorerSelection';

const first = { id: 'first' } as RecordItem;
const second = { id: 'second' } as RecordItem;

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
    act(() => result.current.toggleRecord(first));
    rerender({ scope: 'category' });
    expect(result.current.selectionMode).toBe(false);
    expect(result.current.selectedItems).toEqual([]);
    rerender({ scope: 'product' });
    expect(result.current.selectedItems).toEqual([]);
  });

  it('starts and stays selecting for a relationship picker', () => {
    const { result } = renderHook(() =>
      useExplorerSelection('product', [first, second], {
        alwaysSelecting: true,
      }),
    );
    expect(result.current.selectionMode).toBe(true);
    act(() => result.current.toggleRecord(first));
    act(() => result.current.exitSelectionMode());
    expect(result.current.selectionMode).toBe(true);
    expect(result.current.selectedItems).toEqual([]);
  });

  it('replaces the selected record when only one may be selected', () => {
    const { result } = renderHook(() =>
      useExplorerSelection('product', [first, second], {
        alwaysSelecting: true,
        maximum: 1,
      }),
    );
    act(() => result.current.toggleRecord(first));
    act(() => result.current.toggleRecord(second));
    expect(result.current.selectedItems).toEqual([second]);
    expect(result.current.allLoadedSelected).toBe(false);
    act(() => result.current.toggleLoaded());
    expect(result.current.selectedItems).toEqual([second]);
  });

  it('keeps a relationship picker selection across remounts and adds records', () => {
    const token = '4f0c8e3c-9b0d-4f5a-8f6e-2a1b3c4d5e6f';
    const options = { alwaysSelecting: true, storageToken: token };
    const record = (id: string) =>
      ({
        id,
        blueprint_version: 1,
        schema_outdated: false,
        system_tags: [],
        is_sample: false,
        display: { default: id },
        match_explanations: [],
        preview: {},
        table_values: {},
      }) as unknown as RecordItem;
    const bolt = record('8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f01');
    const nut = record('8d6f1c2a-1b2c-4d3e-8f4a-5b6c7d8e9f02');
    const first = renderHook(() =>
      useExplorerSelection('product', [bolt], options),
    );
    act(() => first.result.current.toggleRecord(bolt));
    first.unmount();

    // Coming back from the create form restores the selection.
    const second = renderHook(() =>
      useExplorerSelection('product', [], options),
    );
    expect(second.result.current.selectedItems.map((item) => item.id)).toEqual([
      bolt.id,
    ]);
    act(() => second.result.current.addRecords([nut, bolt]));
    expect(second.result.current.selectedItems.map((item) => item.id)).toEqual([
      bolt.id,
      nut.id,
    ]);
    second.unmount();

    const single = renderHook(() =>
      useExplorerSelection('product', [], { ...options, maximum: 1 }),
    );
    act(() => single.result.current.addRecords([nut]));
    expect(single.result.current.selectedItems.map((item) => item.id)).toEqual([
      nut.id,
    ]);
    window.sessionStorage.clear();
  });
});

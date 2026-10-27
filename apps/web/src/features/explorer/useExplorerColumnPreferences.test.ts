// @vitest-environment jsdom
import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { explorerColumnIds } from './constants';
import { useExplorerColumnPreferences } from './useExplorerColumnPreferences';

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const columns = ['id', 'name', explorerColumnIds.systemTags];

let storage: Map<string, string>;

beforeEach(() => {
  storage = new Map();
  Object.defineProperty(globalThis, 'localStorage', {
    configurable: true,
    value: {
      getItem: (key: string) => storage.get(key) ?? null,
      removeItem: (key: string) => storage.delete(key),
      setItem: (key: string, value: string) => storage.set(key, value),
    },
  });
});

afterEach(() => storage.clear());

describe('useExplorerColumnPreferences', () => {
  it('restores the default columns, hiding those shown only on request', () => {
    const { result } = renderHook(() =>
      useExplorerColumnPreferences(blueprintId, columns),
    );
    act(() =>
      result.current.update({ hidden: [], order: [...columns].reverse() }),
    );
    act(() => result.current.clear());
    expect(result.current.preferences).toEqual({
      hidden: [explorerColumnIds.systemTags],
      order: columns,
    });
  });

  it('shows columns that appear while mounted as a new viewer sees them', () => {
    const { result, rerender } = renderHook(
      ({ ids }) => useExplorerColumnPreferences(blueprintId, ids),
      { initialProps: { ids: columns } },
    );
    rerender({ ids: [...columns, explorerColumnIds.dataQuality] });
    expect(result.current.preferences).toEqual({
      hidden: [explorerColumnIds.systemTags],
      order: [...columns, explorerColumnIds.dataQuality],
    });
  });
});

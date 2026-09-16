// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import {
  clearExplorerColumnPreferences,
  getExplorerColumnPreferences,
  setExplorerColumnPreferences,
} from './column-preferences';

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const columns = ['id', 'display', 'schema', 'name'];

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

describe('explorer column preferences', () => {
  it('returns the table order when no preferences are stored', () => {
    expect(getExplorerColumnPreferences(blueprintId, columns)).toEqual({
      hidden: [],
      order: columns,
    });
  });

  it('keeps saved choices while reconciling changed table columns', () => {
    setExplorerColumnPreferences(blueprintId, {
      hidden: ['schema', 'removed'],
      order: ['name', 'removed', 'id'],
    });

    expect(getExplorerColumnPreferences(blueprintId, columns)).toEqual({
      hidden: ['schema'],
      order: ['name', 'id', 'display', 'schema'],
    });
  });

  it('clears saved choices for a blueprint', () => {
    setExplorerColumnPreferences(blueprintId, {
      hidden: ['schema'],
      order: ['display', 'id', 'schema', 'name'],
    });
    clearExplorerColumnPreferences(blueprintId);

    expect(getExplorerColumnPreferences(blueprintId, columns)).toEqual({
      hidden: [],
      order: columns,
    });
  });
});

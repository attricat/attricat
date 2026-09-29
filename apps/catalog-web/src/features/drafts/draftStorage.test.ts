// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import { draftEditors, maxDraftLength } from './constants';
import {
  draftStorageKey,
  readDraft,
  removeDraft,
  writeDraft,
  type DraftScope,
} from './draftStorage';

const scope: DraftScope = {
  editor: draftEditors.entityEdit,
  resource: ['entity-1', 'context-1'],
  userId: 'user-1',
  workspaceId: 'workspace-1',
};
const draft = {
  savedAt: '2026-09-29T10:00:00.000Z',
  source: 'source-1',
  value: 'code = "draft"',
};

/** In-memory storage that rejects writes beyond a character quota. */
class QuotaStorage implements Storage {
  private items = new Map<string, string>();

  constructor(private readonly quota: number) {}

  get length() {
    return this.items.size;
  }

  clear() {
    this.items.clear();
  }

  getItem(key: string) {
    return this.items.get(key) ?? null;
  }

  key(index: number) {
    return [...this.items.keys()][index] ?? null;
  }

  removeItem(key: string) {
    this.items.delete(key);
  }

  setItem(key: string, value: string) {
    const used = [...this.items]
      .filter(([itemKey]) => itemKey !== key)
      .reduce((total, [, item]) => total + item.length, 0);
    if (used + value.length > this.quota)
      throw new DOMException('full', 'QuotaExceededError');
    this.items.set(key, value);
  }
}

afterEach(() => {
  vi.restoreAllMocks();
  sessionStorage.clear();
});

describe('draft storage', () => {
  it('scopes keys by workspace, user, editor, and resource context', () => {
    const keys = new Set([
      draftStorageKey(scope),
      draftStorageKey({ ...scope, workspaceId: 'workspace-2' }),
      draftStorageKey({ ...scope, userId: 'user-2' }),
      draftStorageKey({ ...scope, editor: draftEditors.entityCreate }),
      draftStorageKey({ ...scope, resource: ['entity-1', 'context-2'] }),
      draftStorageKey({ ...scope, resource: ['entity-1:context-1'] }),
    ]);
    expect(keys.size).toBe(6);
  });

  it('round-trips a validated draft', () => {
    const key = draftStorageKey(scope);
    expect(writeDraft(key, draft)).toBe(true);
    expect(readDraft(key, z.string())).toEqual(draft);
    removeDraft(key);
    expect(readDraft(key, z.string())).toBeUndefined();
  });

  it('drops malformed, mismatched, and outdated payloads', () => {
    const key = draftStorageKey(scope);
    for (const serialized of [
      'not json',
      JSON.stringify({ format: 0, ...draft }),
      JSON.stringify({ format: 1, ...draft, value: 42 }),
    ]) {
      sessionStorage.setItem(key, serialized);
      expect(readDraft(key, z.string())).toBeUndefined();
      expect(sessionStorage.getItem(key)).toBeNull();
    }
  });

  it('refuses oversized drafts and removes the older copy', () => {
    const key = draftStorageKey(scope);
    writeDraft(key, draft);
    expect(
      writeDraft(key, { ...draft, value: 'x'.repeat(maxDraftLength) }),
    ).toBe(false);
    expect(sessionStorage.getItem(key)).toBeNull();
  });

  it('prunes other drafts when storage is full', () => {
    const key = draftStorageKey(scope);
    const olderKey = draftStorageKey({ ...scope, resource: ['entity-2'] });
    const unrelatedKey = 'catalog.language';
    const serializedLength = JSON.stringify({ format: 1, ...draft }).length;
    const storage = new QuotaStorage(serializedLength * 2);
    vi.spyOn(window, 'sessionStorage', 'get').mockReturnValue(storage);
    storage.setItem(unrelatedKey, 'en');
    writeDraft(olderKey, draft);

    expect(writeDraft(key, draft)).toBe(true);
    expect(storage.getItem(olderKey)).toBeNull();
    expect(storage.getItem(unrelatedKey)).toBe('en');
    expect(readDraft(key, z.string())).toEqual(draft);
  });

  it('treats unavailable storage as having no drafts', () => {
    vi.spyOn(window, 'sessionStorage', 'get').mockImplementation(() => {
      throw new DOMException('denied', 'SecurityError');
    });
    const key = draftStorageKey(scope);
    expect(writeDraft(key, draft)).toBe(false);
    expect(readDraft(key, z.string())).toBeUndefined();
    expect(() => removeDraft(key)).not.toThrow();
  });
});

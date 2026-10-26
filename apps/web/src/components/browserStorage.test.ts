// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from 'vitest';
import { z } from 'zod';
import {
  readStored,
  readStoredJson,
  storageAreas,
  writeStored,
  writeStoredJson,
} from './browserStorage';

const key = 'attricat.test';
const schema = z.object({ order: z.array(z.string()) });

afterEach(() => {
  vi.restoreAllMocks();
  localStorage.clear();
});

describe('browserStorage', () => {
  it('reads back JSON only when it matches the schema', () => {
    writeStoredJson(storageAreas.local, key, { order: ['title'] });
    expect(readStoredJson(storageAreas.local, key, schema)).toEqual({
      order: ['title'],
    });
    writeStored(storageAreas.local, key, '{"order":3}');
    expect(readStoredJson(storageAreas.local, key, schema)).toBeUndefined();
    writeStored(storageAreas.local, key, 'not json');
    expect(readStoredJson(storageAreas.local, key, schema)).toBeUndefined();
    writeStored(storageAreas.local, key, null);
    expect(readStored(storageAreas.local, key)).toBeNull();
  });

  it('behaves as empty when storage throws', () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('denied');
    });
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('denied');
    });
    expect(() => writeStored(storageAreas.session, key, 'x')).not.toThrow();
    expect(readStored(storageAreas.session, key)).toBeNull();
  });
});

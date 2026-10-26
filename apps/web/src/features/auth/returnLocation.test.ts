// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import { returnToStorageKey } from '../../app/storageKeys';
import { takeReturnLocation } from './returnLocation';

afterEach(() => sessionStorage.clear());

describe('takeReturnLocation', () => {
  it('returns a same-origin path once', () => {
    sessionStorage.setItem(returnToStorageKey, '/records/1?tab=history');
    expect(takeReturnLocation()).toBe('/records/1?tab=history');
    expect(takeReturnLocation()).toBeUndefined();
  });

  it.each(['//evil.example', 'https://evil.example', 'records'])(
    'ignores %s',
    (stored) => {
      sessionStorage.setItem(returnToStorageKey, stored);
      expect(takeReturnLocation()).toBeUndefined();
    },
  );
});

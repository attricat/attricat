import { afterEach, describe, expect, it, vi } from 'vitest';
import { randomUuid } from './randomUuid';

const uuidV4Pattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;

describe('randomUuid', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('uses randomUUID when the context provides it', () => {
    expect(randomUuid()).toMatch(uuidV4Pattern);
  });

  it('builds a UUID v4 from random bytes outside a secure context', () => {
    vi.stubGlobal('crypto', {
      getRandomValues: (values: Uint8Array) => values.fill(0xff),
    });

    expect(randomUuid()).toBe('ffffffff-ffff-4fff-bfff-ffffffffffff');
  });
});

import { describe, expect, it } from 'vitest';
import { authQueryKeys } from './queryKeys';

describe('auth query keys', () => {
  it('keeps the session key beneath the auth root', () => {
    expect(authQueryKeys.all()).toEqual(['auth']);
    expect(authQueryKeys.session()).toEqual(['auth', 'session']);
  });

  it('keeps every access check beneath one invalidation root', () => {
    expect(authQueryKeys.access('records.write', 'record', ['a'])).toEqual([
      ...authQueryKeys.accessAll(),
      'records.write',
      'record',
      ['a'],
    ]);
  });
});

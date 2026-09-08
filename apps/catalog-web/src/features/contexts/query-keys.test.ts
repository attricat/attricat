import { describe, expect, it } from 'vitest';
import { contextQueryKeys } from './query-keys';

describe('context query keys', () => {
  it('preserves the shared cache key for context reads', () => {
    expect(contextQueryKeys.all()).toEqual(['attribute-contexts']);
  });
});

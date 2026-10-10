import { describe, expect, it } from 'vitest';
import { dataHealthQueryKeys } from './queryKeys';

describe('data health query keys', () => {
  it('keeps every data-health query beneath its shared root', () => {
    expect(dataHealthQueryKeys.all()).toEqual(['data-health']);
    expect(dataHealthQueryKeys.summary(30)).toEqual([
      'data-health',
      'summary',
      30,
    ]);
    expect(dataHealthQueryKeys.freshness()).toEqual([
      'data-health',
      'freshness',
    ]);
  });
});

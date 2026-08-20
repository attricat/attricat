import { describe, expect, it } from 'vitest';
import { entityQueryKeys } from './query-keys';

describe('entity query keys', () => {
  it('keeps blueprint reads separate from relationship target searches', () => {
    expect(entityQueryKeys.blueprintByCode('category', undefined)).not.toEqual(
      entityQueryKeys.relationshipTargets('category'),
    );
  });
});

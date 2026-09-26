import { describe, expect, it } from 'vitest';
import { ruleQueryKeys } from './queryKeys';

describe('rule query keys', () => {
  it('scopes entity findings beneath the shared findings root', () => {
    expect(ruleQueryKeys.findings('entity-a')).toEqual([
      ...ruleQueryKeys.findings(),
      'entity-a',
    ]);
    expect(ruleQueryKeys.findings('entity-b')).not.toEqual(
      ruleQueryKeys.findings('entity-a'),
    );
  });
});

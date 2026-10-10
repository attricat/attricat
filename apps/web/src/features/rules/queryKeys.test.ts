import { describe, expect, it } from 'vitest';
import { ruleQueryKeys } from './queryKeys';

describe('rule query keys', () => {
  it('scopes record findings beneath the shared findings root', () => {
    expect(ruleQueryKeys.findings('record-a')).toEqual([
      ...ruleQueryKeys.findings(),
      'record-a',
    ]);
    expect(ruleQueryKeys.findings('record-b')).not.toEqual(
      ruleQueryKeys.findings('record-a'),
    );
  });
});

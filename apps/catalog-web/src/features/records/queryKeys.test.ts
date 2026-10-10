import { describe, expect, it } from 'vitest';
import { recordQueryKeys } from './queryKeys';

describe('record query keys', () => {
  it('keeps blueprint reads separate from relationship target searches', () => {
    expect(recordQueryKeys.blueprintByCode('category', undefined)).not.toEqual(
      recordQueryKeys.relationshipTargets('category', ''),
    );
  });

  it('keeps disabled preview keys feature-local and record-specific', () => {
    expect(recordQueryKeys.resolvedPreview('first', undefined)).toEqual([
      'record-resolved-preview',
      'first',
      undefined,
    ]);
    expect(recordQueryKeys.resolvedPreview('first', undefined)).not.toEqual(
      recordQueryKeys.resolvedPreview('second', undefined),
    );
  });

  it('nests record-control reads under the record-control key', () => {
    const root = recordQueryKeys.recordControls('first');
    for (const key of [
      recordQueryKeys.approvals('first'),
      recordQueryKeys.retentionHolds('first'),
      recordQueryKeys.statusTransitions('first', null),
    ])
      expect(key.slice(0, root.length)).toEqual([...root]);
    expect(recordQueryKeys.approvals('first')).not.toEqual(
      recordQueryKeys.retentionHolds('first'),
    );
  });
});

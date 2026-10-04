import { describe, expect, it } from 'vitest';
import { entityQueryKeys } from './queryKeys';

describe('entity query keys', () => {
  it('keeps blueprint reads separate from relationship target searches', () => {
    expect(entityQueryKeys.blueprintByCode('category', undefined)).not.toEqual(
      entityQueryKeys.relationshipTargets('category', ''),
    );
  });

  it('keeps disabled preview keys feature-local and entity-specific', () => {
    expect(entityQueryKeys.resolvedPreview('first', undefined)).toEqual([
      'entity-resolved-preview',
      'first',
      undefined,
    ]);
    expect(entityQueryKeys.resolvedPreview('first', undefined)).not.toEqual(
      entityQueryKeys.resolvedPreview('second', undefined),
    );
  });

  it('nests record-control reads under the record-control key', () => {
    const root = entityQueryKeys.recordControls('first');
    for (const key of [
      entityQueryKeys.approvals('first'),
      entityQueryKeys.retentionHolds('first'),
      entityQueryKeys.statusTransitions('first', null),
    ])
      expect(key.slice(0, root.length)).toEqual([...root]);
    expect(entityQueryKeys.approvals('first')).not.toEqual(
      entityQueryKeys.retentionHolds('first'),
    );
  });
});

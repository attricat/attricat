import type { RecordAuditChange } from './api';

/** Changes that link or unlink a record; their values are record IDs. */
export const relationshipChangeKinds = new Set<
  RecordAuditChange['change_kind']
>(['relationship_add', 'relationship_remove']);

/** The record IDs that relationship changes in `changes` link or unlink. */
export const relationshipChangeRecordIds = (
  changes: readonly RecordAuditChange[],
) =>
  changes.flatMap((change) =>
    relationshipChangeKinds.has(change.change_kind)
      ? [change.before_value, change.after_value].filter(
          (value): value is string => typeof value === 'string',
        )
      : [],
  );

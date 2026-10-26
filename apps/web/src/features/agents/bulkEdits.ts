/** The agent tool that sets the same kind of value on many records. */
export const bulkEditToolName = 'bulk_set_record_values';
/** Run event reporting each record an approved bulk edit applies. */
export const bulkProgressEvent = 'bulk_progress';
/** Records the approval card lists before "show all". */
export const bulkEditPreviewRows = 20;

/** A proposed or previous value as text; `null` when there is none. */
export const formatBulkValue = (value: unknown): string | null => {
  if (value === null || value === undefined) return null;
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean')
    return String(value);
  return JSON.stringify(value);
};

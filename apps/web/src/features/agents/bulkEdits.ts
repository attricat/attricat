import { z } from 'zod';

/** The agent tool that sets the same kind of value on many records. */
export const bulkEditToolName = 'bulk_set_record_values';
/** Run event reporting each record an approved bulk edit applies. */
export const bulkProgressEvent = 'bulk_progress';
/** Records the approval card lists before "show all". */
export const bulkEditPreviewRows = 20;

const id = z.string().uuid();

const namedCodeSchema = z.object({ code: z.string(), name: z.string() });

/** The resolved proposal a bulk edit's approval covers. */
export const bulkEditProposalSchema = z.object({
  blueprint: namedCodeSchema.nullable(),
  attributes: z.array(namedCodeSchema),
  records: z.array(
    z.object({
      record_id: id,
      blueprint_code: z.string(),
      label: z.string().nullable(),
      values: z.array(
        z.object({
          attribute_code: z.string(),
          context_id: id.nullable(),
          value: z.unknown(),
          previous: z.unknown(),
        }),
      ),
    }),
  ),
});

export const bulkEditResultSchema = z.object({
  applied: z.array(id),
  failed: z.array(
    z.object({ record_id: id, code: z.string(), message: z.string() }),
  ),
  not_run: z.array(id),
});

export const bulkProgressSchema = z.object({
  run_id: id,
  payload: z.object({
    tool_call_id: id,
    record_id: id,
    state: z.enum(['applied', 'failed']),
    applied: z.number().int().nonnegative(),
    total: z.number().int().positive(),
  }),
});

export type BulkEditProposal = z.infer<typeof bulkEditProposalSchema>;
export type BulkEditResult = z.infer<typeof bulkEditResultSchema>;
export type BulkProgress = z.infer<typeof bulkProgressSchema>['payload'] & {
  run_id: string;
};

/** A proposed or previous value as text; `null` when there is none. */
export const formatBulkValue = (value: unknown): string | null => {
  if (value === null || value === undefined) return null;
  if (typeof value === 'string') return value;
  if (typeof value === 'number' || typeof value === 'boolean')
    return String(value);
  return JSON.stringify(value);
};

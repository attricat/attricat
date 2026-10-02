import { z } from 'zod';
import { extensionRunFailures, extensionRunStatuses } from './constants';

/**
 * Optional, extension-reported domain outcome. Execution status stays
 * separate: a completed run can still report per-entity failures.
 */
const progressSchema = z.record(z.string(), z.unknown());

export const extensionRunSchema = z.object({
  id: z.uuid(),
  extension_id: z.string().min(1),
  contribution_id: z.string().nullable(),
  operation_id: z.string().min(1),
  status: z.enum(extensionRunStatuses),
  progress: progressSchema,
  failure: z.enum(extensionRunFailures).nullable(),
  can_cancel: z.boolean(),
  selection_count: z.number().int().nonnegative(),
  blueprint_id: z.uuid().nullable(),
  blueprint_version: z.number().int().positive().nullable(),
  context_id: z.uuid().nullable(),
  created_at: z.string(),
  completed_at: z.string().nullable(),
  cancelled_at: z.string().nullable(),
  outputs_expire_at: z.string().nullable(),
  outputs_expired: z.boolean(),
});
export type ExtensionRun = z.infer<typeof extensionRunSchema>;

export const extensionRunArtifactSchema = z.object({
  id: z.uuid(),
  name: z.string().nullable(),
  media_type: z.string(),
  content_length: z.number().int().nonnegative(),
  completed_at: z.string().nullable(),
});
export type ExtensionRunArtifact = z.infer<typeof extensionRunArtifactSchema>;

export const extensionRunDetailSchema = extensionRunSchema.extend({
  artifacts: z.array(extensionRunArtifactSchema),
});
export type ExtensionRunDetail = z.infer<typeof extensionRunDetailSchema>;

export const startedExtensionRunSchema = z.object({ run_id: z.uuid() });

/** Optional counts an extension may report for its domain outcome. */
export const runOutcomeSchema = z.object({
  succeeded: z.number().int().nonnegative().optional(),
  failed: z.number().int().nonnegative().optional(),
  skipped: z.number().int().nonnegative().optional(),
});
export const runProgressCountsSchema = z.object({
  completed: z.number().int().nonnegative(),
  total: z.number().int().positive(),
});

import { z } from 'zod';
import { revisionStatusSchema } from '../../api/revisionStatus';
import {
  findingSeverities,
  findingStateFilters,
  findingStates,
  ruleRunStatuses,
} from './constants';

export const ruleSchema = z.object({
  id: z.uuid(),
  blueprint_id: z.uuid(),
  blueprint_version: z.number().int().positive(),
  context_id: z.uuid().nullable(),
  code: z.string(),
  name: z.string(),
  version: z.number().int().positive(),
  status: revisionStatusSchema,
  definition: z.string(),
  definition_hash: z.string(),
  compiled_plan: z.unknown(),
  published_at: z.string().nullable(),
  created_at: z.string(),
  enabled_version: z.number().int().positive().nullable(),
});
export const ruleRunSchema = z.object({
  id: z.uuid(),
  rule_id: z.uuid(),
  rule_version: z.number().int().positive(),
  source: z.enum(['manual', 'schedule', 'event', 'post_import']),
  dry_run: z.boolean(),
  scope_record_id: z.uuid().nullable(),
  status: z.enum(ruleRunStatuses),
  candidate_cursor: z.uuid().nullable(),
  candidates_evaluated: z.number().int().nonnegative(),
  findings_created: z.number().int().nonnegative(),
  findings_resolved: z.number().int().nonnegative(),
  attempts: z.number().int().nonnegative(),
  last_error: z.string().nullable(),
  completed_at: z.string().nullable(),
  created_at: z.string(),
});
export const findingSchema = z.object({
  id: z.uuid(),
  rule_id: z.uuid(),
  rule_version: z.number().int().positive(),
  record_id: z.uuid(),
  context_id: z.uuid().nullable(),
  severity: z.enum(findingSeverities),
  message: z.string(),
  evidence: z.unknown(),
  state: z.enum(findingStates),
  acknowledged_at: z.string().nullable(),
  resolved_at: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});
/** The evidence fields the record page shows; rules may add others. */
export const findingEvidenceSchema = z.object({
  contexts: z.array(z.string()).optional(),
  attribute_code: z.string().optional(),
});
/** The findings page's filters, kept in its URL. Unset filters match all. */
export const findingsSearchSchema = z.object({
  severity: z
    .enum(Object.values(findingSeverities))
    .optional()
    .catch(undefined),
  state: z.enum(Object.values(findingStateFilters)).optional().catch(undefined),
  rule: z.uuid().optional().catch(undefined),
  blueprint: z.uuid().optional().catch(undefined),
  context: z.uuid().optional().catch(undefined),
});
export type FindingsSearch = z.infer<typeof findingsSearchSchema>;
export type Rule = z.infer<typeof ruleSchema>;
export type RuleRun = z.infer<typeof ruleRunSchema>;
export type Finding = z.infer<typeof findingSchema>;

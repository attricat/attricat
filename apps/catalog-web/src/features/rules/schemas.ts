import { z } from 'zod';

export const ruleSchema = z.object({
  id: z.uuid(),
  blueprint_id: z.uuid(),
  blueprint_version: z.number().int().positive(),
  context_id: z.uuid().nullable(),
  code: z.string(),
  name: z.string(),
  version: z.number().int().positive(),
  status: z.enum(['draft', 'published']),
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
  scope_entity_id: z.uuid().nullable(),
  status: z.string(),
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
  entity_id: z.uuid(),
  context_id: z.uuid().nullable(),
  severity: z.string(),
  message: z.string(),
  evidence: z.unknown(),
  state: z.enum(['open', 'acknowledged', 'resolved']),
  acknowledged_at: z.string().nullable(),
  resolved_at: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
});
export type Rule = z.infer<typeof ruleSchema>;
export type RuleRun = z.infer<typeof ruleRunSchema>;
export type Finding = z.infer<typeof findingSchema>;

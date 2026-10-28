import { z } from 'zod';
import {
  findingSeverities,
  findingStateFilters,
  findingStates,
  ruleInapplicabilities,
  ruleTriggerTypes,
} from './constants';
import { runStatusSchema } from '../../api/runStatus';

/** A rule of a blueprint: its current definition and whether it runs. */
export const ruleSchema = z.object({
  id: z.uuid(),
  blueprint_id: z.uuid(),
  context_id: z.uuid().nullable(),
  code: z.string(),
  name: z.string(),
  /** The current revision. */
  version: z.number().int().positive(),
  definition: z.string(),
  definition_hash: z.string(),
  compiled_plan: z.unknown(),
  enabled: z.boolean(),
  created_at: z.string(),
  /** When the current revision was saved. */
  updated_at: z.string(),
});
/** A blueprint revision whose records a rule skips, and why. */
export const revisionNotApplicableSchema = z.object({
  blueprint_version: z.number().int().positive(),
  reason: z.enum(ruleInapplicabilities),
  message: z.string(),
});
/** A rule with the blueprint revisions it does not fit. */
export const ruleDetailsSchema = ruleSchema.extend({
  not_applicable: z.array(revisionNotApplicableSchema),
  /** Across the workspace; left out for readers of a single record. */
  open_findings: z.number().int().nonnegative().optional(),
  acknowledged_findings: z.number().int().nonnegative().optional(),
});
/** One saved definition of a rule, from its changelog. */
export const ruleRevisionSchema = z.object({
  rule_id: z.uuid(),
  version: z.number().int().positive(),
  name: z.string(),
  definition: z.string(),
  definition_hash: z.string(),
  created_by_user_id: z.uuid().nullable(),
  created_at: z.string(),
});
/** The rule input that validation and creation accept. */
export const createRuleSchema = z.object({
  blueprint_id: z.uuid(),
  context_id: z.uuid().nullable(),
  definition: z.string(),
});
/** A changed definition, saved as the rule's new current revision. */
export const updateRuleSchema = z.object({
  definition: z.string(),
  accept_existing_violations: z.boolean().optional(),
});
/** An enabled rule that a newly published blueprint revision does not fit. */
export const ruleWarningSchema = z.object({
  rule_id: z.uuid(),
  code: z.string(),
  name: z.string(),
  reason: z.enum(ruleInapplicabilities),
  message: z.string(),
});
const ruleTriggerSchema = z.discriminatedUnion('type', [
  z.object({ type: z.literal(ruleTriggerTypes.manual) }),
  z.object({
    type: z.literal(ruleTriggerTypes.schedule),
    cron: z.string(),
    timezone: z.string(),
  }),
  z.object({ type: z.literal(ruleTriggerTypes.event), event_type: z.string() }),
  z.object({ type: z.literal(ruleTriggerTypes.postImport) }),
]);
/** A definition that passed validation, as the editor previews it. */
export const compiledRuleSchema = z.object({
  code: z.string(),
  name: z.string(),
  severity: z.enum(findingSeverities),
  triggers: z.array(ruleTriggerSchema),
  predicate: z.object({ type: z.string() }).loose(),
  enforcement: z
    .object({
      on_save: z.boolean(),
      transitions: z
        .array(
          z.object({
            attribute_code: z.string(),
            from: z.string().nullish(),
            to: z.string(),
          }),
        )
        .default([]),
    })
    .nullish(),
});
export const ruleRunSchema = z.object({
  id: z.uuid(),
  rule_id: z.uuid(),
  rule_version: z.number().int().positive(),
  /** The definition the run checked; a dry run may check an unsaved one. */
  definition_hash: z.string(),
  source: z.enum(ruleTriggerTypes),
  dry_run: z.boolean(),
  scope_record_id: z.uuid().nullable(),
  status: runStatusSchema,
  candidate_cursor: z.uuid().nullable(),
  candidates_evaluated: z.number().int().nonnegative(),
  /** Candidates on a blueprint revision the rule does not fit. */
  candidates_skipped: z.number().int().nonnegative(),
  findings_created: z.number().int().nonnegative(),
  findings_resolved: z.number().int().nonnegative(),
  attempts: z.number().int().nonnegative(),
  last_error: z.string().nullable(),
  /** The run stopped at its record limit with records left unchecked. */
  truncated: z.boolean(),
  completed_at: z.string().nullable(),
  created_at: z.string(),
});
/** A run with, for a dry run, the first records it would flag. */
export const ruleRunDetailsSchema = ruleRunSchema.extend({
  dry_run_failures: z.array(
    z.object({
      record_id: z.uuid(),
      message: z.string(),
      evidence: z.unknown(),
    }),
  ),
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
/** The run history's filters, kept in its URL. Unset filters match all. */
export const runsSearchSchema = z.object({
  rule: z.uuid().optional().catch(undefined),
});
export type RunsSearch = z.infer<typeof runsSearchSchema>;
const comparedVersionSchema = z.coerce
  .number()
  .int()
  .positive()
  .optional()
  .catch(undefined);
/**
 * The two revisions a rule's changelog compares, kept in its URL. Without
 * `from`, `to` is compared with the revision before it; without `to`, the
 * current revision is shown.
 */
export const ruleComparisonSearchSchema = z.object({
  from: comparedVersionSchema,
  to: comparedVersionSchema,
});
export type RuleComparisonSearch = z.infer<typeof ruleComparisonSearchSchema>;
export type Rule = z.infer<typeof ruleSchema>;
export type RuleDetails = z.infer<typeof ruleDetailsSchema>;
export type RuleRevision = z.infer<typeof ruleRevisionSchema>;
export type RuleWarning = z.infer<typeof ruleWarningSchema>;
export type UpdateRule = z.infer<typeof updateRuleSchema>;
export type RuleRun = z.infer<typeof ruleRunSchema>;
export type RuleRunDetails = z.infer<typeof ruleRunDetailsSchema>;
export type Finding = z.infer<typeof findingSchema>;
export type CreateRule = z.infer<typeof createRuleSchema>;
export type CompiledRule = z.infer<typeof compiledRuleSchema>;
export type RuleTrigger = z.infer<typeof ruleTriggerSchema>;

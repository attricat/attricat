import { z } from 'zod';
import { maximumStaleAfterDays, minimumStaleAfterDays } from './constants';

export const dataHealthSearchSchema = z.object({
  staleAfterDays: z.coerce
    .number()
    .int()
    .min(minimumStaleAfterDays)
    .max(maximumStaleAfterDays)
    .optional()
    .catch(undefined),
});

export const dataHealthSummarySchema = z.object({
  active_entities: z.number().int().nonnegative(),
  entity_blueprints: z.number().int().nonnegative(),
  contexts: z.number().int().nonnegative(),
  outdated_entities: z.number().int().nonnegative(),
  stale_entities: z.number().int().nonnegative(),
  deleted_relationship_targets: z.number().int().nonnegative(),
});

export const blueprintHealthSchema = z.object({
  code: z.string(),
  name: z.string(),
  current_version: z.number().int().positive(),
  active_entities: z.number().int().nonnegative(),
  outdated_entities: z.number().int().nonnegative(),
  stale_entities: z.number().int().nonnegative(),
  oldest_updated_at: z.string().datetime().nullable(),
  newest_updated_at: z.string().datetime().nullable(),
});

export const freshnessBandSchema = z.object({
  label: z.string(),
  entities: z.number().int().nonnegative(),
});

export const completenessHealthSchema = z.object({
  code: z.string(),
  name: z.string(),
  current_version: z.number().int().positive(),
  active_entities: z.number().int().nonnegative(),
  outdated_entities: z.number().int().nonnegative(),
  default_complete_entities: z.number().int().nonnegative(),
});

export const contextHealthSchema = z.object({
  code: z.string(),
  direct_entities: z.number().int().nonnegative(),
  direct_values: z.number().int().nonnegative(),
});

export const relationshipHealthSchema = z.object({
  attribute_code: z.string(),
  source_blueprint: z.string(),
  active_edges: z.number().int().nonnegative(),
  deleted_targets: z.number().int().nonnegative(),
});

export const storageHealthSchema = z.object({
  table: z.string(),
  bytes: z.number().int().nonnegative(),
});

export type DataHealthSearch = z.infer<typeof dataHealthSearchSchema>;
export type DataHealthSummary = z.infer<typeof dataHealthSummarySchema>;
export type BlueprintHealth = z.infer<typeof blueprintHealthSchema>;
export type FreshnessBand = z.infer<typeof freshnessBandSchema>;
export type CompletenessHealth = z.infer<typeof completenessHealthSchema>;
export type ContextHealth = z.infer<typeof contextHealthSchema>;
export type RelationshipHealth = z.infer<typeof relationshipHealthSchema>;
export type StorageHealth = z.infer<typeof storageHealthSchema>;

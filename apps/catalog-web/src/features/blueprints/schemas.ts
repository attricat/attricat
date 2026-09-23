import { z } from 'zod';
import {
  attributeSchema as entityAttributeSchema,
  blueprintSchema as entityBlueprintSchema,
} from '../entities/schemas';

const jsonValueSchema: z.ZodType<unknown> = z.lazy(() =>
  z.union([
    z.string(),
    z.number(),
    z.boolean(),
    z.null(),
    z.array(jsonValueSchema),
    z.record(z.string(), jsonValueSchema),
  ]),
);

export const blueprintSchema = z.object({
  id: z.uuid(),
  code: z.string(),
  name: z.string(),
  kind: z.string(),
  version: z.number().int().positive(),
  includes: jsonValueSchema,
  views: entityBlueprintSchema.shape.views,
  entity_schema: entityBlueprintSchema.shape.entity_schema,
  status: z.enum(['draft', 'published']),
  published_at: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
  deleted_at: z.string().nullable(),
  definition: z.string(),
  definition_hash: z.string(),
});

export const attributeSchema = entityAttributeSchema.extend({
  id: z.uuid(),
  blueprint_id: z.uuid(),
  blueprint_version: z.number().int().positive(),
  tags: z.array(z.string()),
  context_fallback: z.enum(['default', 'none']),
  context_editable: z.enum(['all', 'default']),
  position: z.number().int(),
  created_at: z.string(),
  updated_at: z.string(),
  deleted_at: z.string().nullable(),
});

export const blueprintWithAttributesSchema = z.object({
  blueprint: blueprintSchema,
  attributes: z.array(attributeSchema),
});

export type Blueprint = z.infer<typeof blueprintSchema>;
export type Attribute = z.infer<typeof attributeSchema>;
export type BlueprintWithAttributes = z.infer<
  typeof blueprintWithAttributesSchema
>;

export const blueprintEntityPublicationSummarySchema = z.object({
  entity_count: z.number().int().nonnegative(),
  channel_count: z.number().int().nonnegative(),
  publication_count: z.number().int().nonnegative(),
});

export const blueprintMigrationBatchSchema = z.object({
  id: z.uuid(),
  blueprint_id: z.uuid(),
  target_version: z.number().int().positive(),
  status: z.enum([
    'draft',
    'queued',
    'running',
    'completed',
    'failed',
    'superseded',
  ]),
  removal_policy: z.record(z.string(), z.unknown()).default({}),
  created_at: z.string(),
  started_at: z.string().nullable(),
  completed_at: z.string().nullable(),
});

export type BlueprintMigrationBatch = z.infer<
  typeof blueprintMigrationBatchSchema
>;

export const blueprintMigrationBatchStatusSchema =
  blueprintMigrationBatchSchema.extend({
    total_entities: z.number().int().nonnegative(),
    processed_entities: z.number().int().nonnegative(),
    migrated_entities: z.number().int().nonnegative(),
    needs_input_entities: z.number().int().nonnegative(),
    failed_entities: z.number().int().nonnegative(),
  });

export type BlueprintMigrationBatchStatus = z.infer<
  typeof blueprintMigrationBatchStatusSchema
>;

export const blueprintMigrationImpactSchema = z.object({
  eligible_entities: z.number().int().nonnegative(),
  removed_attribute_codes: z.array(z.string()),
  entities_with_removed_values: z.number().int().nonnegative(),
  removed_values: z.number().int().nonnegative(),
  requires_removal_disposition: z.boolean(),
});

export type BlueprintMigrationImpact = z.infer<
  typeof blueprintMigrationImpactSchema
>;

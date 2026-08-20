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
  entity_schema: jsonValueSchema.nullable(),
  status: z.string(),
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

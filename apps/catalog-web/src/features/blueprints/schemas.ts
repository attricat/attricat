import { z } from 'zod';

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
  display: jsonValueSchema,
  views: jsonValueSchema,
  entity_schema: jsonValueSchema.nullable(),
  status: z.string(),
  published_at: z.string().nullable(),
  created_at: z.string(),
  updated_at: z.string(),
  deleted_at: z.string().nullable(),
  definition: z.string(),
  definition_hash: z.string(),
});

export const attributeSchema = z.object({
  id: z.uuid(),
  blueprint_id: z.uuid(),
  blueprint_version: z.number().int().positive(),
  code: z.string(),
  value_type: z.string(),
  value_schema: jsonValueSchema.nullable(),
  target_blueprint_code: z.string().nullable(),
  tags: jsonValueSchema,
  context_fallback: z.string(),
  context_editable: z.string(),
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

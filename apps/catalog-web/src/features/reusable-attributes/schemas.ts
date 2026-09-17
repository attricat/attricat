import { z } from 'zod';

export const reusableAttributeValueTypeSchema = z.enum([
  'string',
  'number',
  'integer',
  'boolean',
  'date',
  'datetime',
  'time',
  'relationship',
  'file',
]);

export const reusableAttributeSchema = z.object({
  id: z.uuid(),
  definition_id: z.uuid(),
  namespace: z.string(),
  code: z.string(),
  name: z.string(),
  version: z.number().int().positive(),
  value_type: reusableAttributeValueTypeSchema,
  value_schema: z
    .union([z.record(z.string(), z.unknown()), z.boolean()])
    .nullable(),
  default_value: z.json().nullable(),
  file_policy: z.unknown().nullable(),
  target_blueprint_code: z.string().nullable(),
  cardinality: z.enum(['one', 'many']).nullable(),
  target_cardinality: z.enum(['one', 'many']).nullable(),
  tags: z.array(z.string()),
  context_fallback: z.enum(['default', 'none']),
  context_editable: z.enum(['all', 'default']),
  readonly: z.boolean(),
  searchable: z.boolean(),
  facetable: z.boolean(),
  status: z.enum(['draft', 'published']),
  published_at: z.string().datetime().nullable(),
});

export const reusableAttributeGroupSchema = z.object({
  id: z.uuid(),
  code: z.string(),
  name: z.string(),
  position: z.number().int(),
  reusable_attribute_revision_ids: z.array(z.uuid()),
});

export const createReusableAttributeSchema = z.object({
  namespace: z.string().trim().min(1),
  code: z.string().trim().min(1),
  name: z.string().trim().min(1),
  value_type: reusableAttributeValueTypeSchema,
  value_schema: z
    .union([z.record(z.string(), z.unknown()), z.boolean()])
    .nullable()
    .optional(),
  default_value: z.json().nullable().optional(),
  file_policy: z.unknown().nullable().optional(),
  target_blueprint_code: z.string().trim().min(1).nullable().optional(),
  cardinality: z.enum(['one', 'many']).nullable().optional(),
  target_cardinality: z.enum(['one', 'many']).nullable().optional(),
  tags: z.array(z.string()),
  context_fallback: z.enum(['default', 'none']),
  context_editable: z.enum(['all', 'default']),
  readonly: z.boolean(),
  searchable: z.boolean(),
  facetable: z.boolean(),
});

export const createReusableAttributeGroupSchema = z.object({
  code: z.string().trim().min(1),
  name: z.string().trim().min(1),
  position: z.number().int().nonnegative(),
  reusable_attribute_revision_ids: z.array(z.uuid()),
});

export type ReusableAttribute = z.infer<typeof reusableAttributeSchema>;
export type ReusableAttributeGroup = z.infer<
  typeof reusableAttributeGroupSchema
>;
export type CreateReusableAttribute = z.input<
  typeof createReusableAttributeSchema
>;

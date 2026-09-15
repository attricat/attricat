import { z } from 'zod';

export const maximumAttributeFilters = 20;

const attributeFilterOperatorSchema = z.enum([
  'eq',
  'contains',
  'starts_with',
  'gt',
  'gte',
  'lt',
  'lte',
]);

const attributeFilterSearchSchema = z.object({
  field: z.string().trim().min(1),
  operator: attributeFilterOperatorSchema,
  value: z.union([z.string(), z.number().finite(), z.boolean()]),
});

const relationshipFacetSearchSchema = z.object({
  field: z.string().trim().min(1),
  selectedIds: z.array(z.string().uuid()).optional(),
  targetBlueprint: z.string().trim().min(1).optional(),
});

export const explorerSearchSchema = z.object({
  blueprint: z.string().trim().min(1).optional().catch(undefined),
  version: z.coerce.number().int().positive().optional().catch(undefined),
  allVersions: z.boolean().optional().catch(undefined),
  query: z.string().trim().min(1).optional().catch(undefined),
  context: z.string().trim().min(1).optional().catch(undefined),
  locked: z.boolean().optional().catch(undefined),
  sort: z
    .object({
      field: z.string().trim().min(1),
      direction: z.enum(['asc', 'desc']),
    })
    .optional()
    .catch(undefined),
  relationshipFacets: z
    .array(relationshipFacetSearchSchema)
    .optional()
    .catch(undefined),
  attributeFilters: z
    .array(attributeFilterSearchSchema)
    .max(maximumAttributeFilters)
    .optional()
    .catch(undefined),
});

export type AttributeFilter = z.infer<typeof attributeFilterSearchSchema>;
export type ExplorerSearch = z.infer<typeof explorerSearchSchema>;

export const parseExplorerSearch = (
  input: Record<string, unknown>,
): ExplorerSearch => explorerSearchSchema.parse(input);

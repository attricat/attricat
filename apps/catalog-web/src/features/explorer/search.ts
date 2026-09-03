import { z } from 'zod';

const relationshipFacetSearchSchema = z.object({
  field: z.string().trim().min(1),
  hierarchy: z.string().trim().min(1).optional(),
  context: z.string().trim().min(1).optional(),
  selectedIds: z.array(z.string().uuid()).optional(),
});

export const explorerSearchSchema = z.object({
  blueprint: z.string().trim().min(1).optional().catch(undefined),
  version: z.coerce.number().int().positive().optional().catch(undefined),
  query: z.string().trim().min(1).optional().catch(undefined),
  relationshipFacets: z
    .array(relationshipFacetSearchSchema)
    .optional()
    .catch(undefined),
});

export type ExplorerSearch = z.infer<typeof explorerSearchSchema>;

export const parseExplorerSearch = (
  input: Record<string, unknown>,
): ExplorerSearch => explorerSearchSchema.parse(input);

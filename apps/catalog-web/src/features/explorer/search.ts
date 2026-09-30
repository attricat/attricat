import { z } from 'zod';
import { entitySearchFilterSchema } from '../entities/schemas';
import { maximumAttributeFilters, sortDirections } from './constants';
import { entityIdsQuery } from './queryLanguage';

const attributeFilterSearchSchema = entitySearchFilterSchema.extend({
  field: z.string().trim().min(1),
});

const relationshipFacetSearchSchema = z.object({
  field: z.string().trim().min(1),
  selectedIds: z.array(z.string().uuid()).optional(),
  targetBlueprint: z.string().trim().min(1).optional(),
});

export const explorerSearchSchema = z.object({
  savedView: z.uuid().optional().catch(undefined),
  viewState: z.uuid().optional().catch(undefined),
  sourceView: z.uuid().optional().catch(undefined),
  blueprint: z.string().trim().min(1).optional().catch(undefined),
  version: z.coerce.number().int().positive().optional().catch(undefined),
  allVersions: z.boolean().optional().catch(undefined),
  query: z.string().trim().min(1).optional().catch(undefined),
  context: z.string().trim().min(1).optional().catch(undefined),
  locked: z.boolean().optional().catch(undefined),
  sort: z
    .object({
      field: z.string().trim().min(1),
      direction: z.enum([sortDirections.ascending, sortDirections.descending]),
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

export const inlineExplorerSearch = (
  search: ExplorerSearch,
): ExplorerSearch => {
  const state = { ...search };
  delete state.savedView;
  delete state.viewState;
  delete state.sourceView;
  return state;
};

export const inlineExplorerSearchParams = (search: ExplorerSearch) => {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(inlineExplorerSearch(search))) {
    if (value !== undefined) {
      params.set(
        key,
        typeof value === 'object' ? JSON.stringify(value) : String(value),
      );
    }
  }
  return params;
};

export const parseExplorerSearch = (
  input: Record<string, unknown>,
): ExplorerSearch => explorerSearchSchema.parse(input);

export type ExplorerSort = NonNullable<ExplorerSearch['sort']>;

/**
 * A fresh search of the same blueprint and version scope that matches only
 * the given entities; filters and facets are dropped so none can hide them.
 */
export const entitySelectionSearch = (
  search: ExplorerSearch,
  entityIds: string[],
): ExplorerSearch => ({
  blueprint: search.blueprint,
  version: search.version,
  allVersions: search.allVersions,
  context: search.context,
  locked: search.locked,
  sort: search.sort,
  query: entityIdsQuery(entityIds),
});

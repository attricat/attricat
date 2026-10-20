import { z } from 'zod';
import { recordSearchFilterSchema } from '../records/schemas';
import { maximumAttributeFilters, sortDirections } from './constants';
import { recordIdsQuery } from './queryLanguage';

const attributeFilterSearchSchema = recordSearchFilterSchema.safeExtend({
  field: z.string().trim().min(1),
});

const relationshipFacetSearchSchema = z.object({
  field: z.string().trim().min(1),
  selectedIds: z.array(z.string().uuid()).optional(),
  targetBlueprint: z.string().trim().min(1).optional(),
  /** Self-targeting field of the target; selections then match descendants. */
  hierarchyField: z.string().trim().min(1).optional(),
  /**
   * `field` is the searched blueprint's own hierarchy field: a selection
   * matches the selected records and their descendants.
   */
  ownTree: z.literal(true).optional(),
});

/** The relationship field the browse rail lists, or `false` when it is hidden. */
export const browseBySchema = z.union([
  z.string().trim().min(1),
  z.literal(false),
]);

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
  browseBy: browseBySchema.optional().catch(undefined),
});

export type AttributeFilter = z.infer<typeof attributeFilterSearchSchema>;
export type BrowseBy = z.infer<typeof browseBySchema>;
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

/**
 * The Explorer route's URL: the search plus the record open in the side
 * panel. The panel is not part of the search, so saved views never store it.
 */
const explorerRouteSearchSchema = explorerSearchSchema.extend({
  record: z.uuid().optional().catch(undefined),
});

export type ExplorerRouteSearch = z.infer<typeof explorerRouteSearchSchema>;

export const parseExplorerRouteSearch = (
  input: Record<string, unknown>,
): ExplorerRouteSearch => explorerRouteSearchSchema.parse(input);

/** The search itself, without the record open in the panel. */
export const withoutPanelRecord = (
  routeSearch: ExplorerRouteSearch,
): ExplorerSearch => {
  const search = { ...routeSearch };
  delete search.record;
  return search;
};

export type ExplorerSort = NonNullable<ExplorerSearch['sort']>;

/**
 * A fresh search of the same blueprint and version scope that matches only
 * the given records; filters and facets are dropped so none can hide them.
 */
export const recordSelectionSearch = (
  search: ExplorerSearch,
  recordIds: string[],
): ExplorerSearch => ({
  blueprint: search.blueprint,
  version: search.version,
  allVersions: search.allVersions,
  context: search.context,
  locked: search.locked,
  sort: search.sort,
  query: recordIdsQuery(recordIds),
});

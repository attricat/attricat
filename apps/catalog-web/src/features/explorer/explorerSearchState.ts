import { recordQueryKeys } from '../records/queryKeys';
import { explorerSortFields, sortDirections } from './constants';
import { isRelationshipPath } from './explorerTableColumns';
import type {
  ExplorerRelationshipFacet,
  RelationshipFacetUpdate,
  RelationshipFilterAttribute,
} from './relationshipFilterTypes';
import type { ExplorerSearch, ExplorerSort } from './search';

export type RelationshipFilter = {
  field: string;
  selected_target_ids: string[];
};

/**
 * Relationship paths only resolve against one blueprint version, so changing
 * the version scope drops relationship facets and path-based filters/sorts.
 */
export const withoutRelationshipPathScope = (
  search: ExplorerSearch,
): Partial<ExplorerSearch> => ({
  relationshipFacets: undefined,
  attributeFilters: search.attributeFilters?.filter(
    (filter) => !isRelationshipPath(filter.field),
  ),
  sort:
    search.sort && isRelationshipPath(search.sort.field)
      ? undefined
      : search.sort,
});

/**
 * Facets can reference relationship paths saved in the URL that are not
 * direct relationships of the selected blueprint revision.
 */
export const relationshipFacetSources = (
  relationshipFields: RelationshipFilterAttribute[],
  search: ExplorerSearch,
): RelationshipFilterAttribute[] => [
  ...relationshipFields,
  ...(search.relationshipFacets ?? []).flatMap(
    (facet): RelationshipFilterAttribute[] =>
      facet.targetBlueprint &&
      !relationshipFields.some((field) => field.code === facet.field)
        ? [
            {
              cardinality: 'many',
              code: facet.field,
              target_blueprint_code: facet.targetBlueprint,
              value_type: 'relationship',
            },
          ]
        : [],
  ),
];

export const selectedRelationshipFacets = (
  sources: RelationshipFilterAttribute[],
  search: ExplorerSearch,
): ExplorerRelationshipFacet[] =>
  sources
    .map((sourceRelationship) => ({
      selectedIds:
        search.relationshipFacets?.find(
          (facet) => facet.field === sourceRelationship.code,
        )?.selectedIds ?? [],
      sourceRelationship,
    }))
    .filter((facet) => facet.selectedIds.length > 0);

export const relationshipFiltersFromSearch = (
  search: ExplorerSearch,
): RelationshipFilter[] =>
  (search.relationshipFacets ?? []).flatMap((facet) =>
    facet.selectedIds?.length
      ? [{ field: facet.field, selected_target_ids: facet.selectedIds }]
      : [],
  );

export const updateRelationshipFacets = (
  current: ExplorerSearch['relationshipFacets'] = [],
  field: string,
  updates: RelationshipFacetUpdate,
) => {
  const remaining = current.filter((facet) => facet.field !== field);
  const nextFacet = {
    field,
    ...current.find((facet) => facet.field === field),
    ...updates,
  };
  const relationshipFacets =
    updates.selectedIds?.length === 0 ? remaining : [...remaining, nextFacet];
  return relationshipFacets.length ? relationshipFacets : undefined;
};

/** Publication sorting needs the context whose publication state is sorted. */
export const requestSort = (
  sort: ExplorerSort | undefined,
  contextCode: string,
) =>
  sort?.field === explorerSortFields.publicationStatus
    ? { ...sort, context_code: contextCode }
    : sort;

export const toggledSort = (
  sort: ExplorerSort | undefined,
  field: string,
): ExplorerSort => ({
  field,
  direction:
    sort?.field === field && sort.direction === sortDirections.ascending
      ? sortDirections.descending
      : sortDirections.ascending,
});

export const explorerResultsQueryKey = (
  search: ExplorerSearch,
  relationshipFilters: RelationshipFilter[],
  version: number | undefined,
  sort: ReturnType<typeof requestSort>,
  contextCode: string,
) =>
  recordQueryKeys.search({
    allVersions: Boolean(search.allVersions),
    blueprint: search.blueprint,
    contextCode,
    filters: search.attributeFilters,
    query: search.query,
    relationshipFilters,
    sort,
    version,
  });

/** Merges submitted search form values into the current Explorer search. */
export const submittedExplorerSearch = (
  search: ExplorerSearch,
  value: ExplorerSearch,
) => {
  const keepsBlueprint = value.blueprint === search.blueprint;
  const scopeChanged =
    value.version !== search.version ||
    Boolean(value.allVersions) !== Boolean(search.allVersions);
  const nextSearch: ExplorerSearch = {
    ...(keepsBlueprint
      ? search
      : { relationshipFacets: undefined, attributeFilters: undefined }),
    version: undefined,
    allVersions: undefined,
    ...value,
    ...(scopeChanged ? withoutRelationshipPathScope(search) : {}),
  };
  return { keepsBlueprint, nextSearch, scopeChanged };
};

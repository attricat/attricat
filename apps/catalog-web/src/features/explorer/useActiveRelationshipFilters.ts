import { useQueries } from '@tanstack/react-query';
import { getRelationshipTreeFacetChildren } from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import type { ExplorerRelationshipFacet } from './ExplorerFacetSidebar';
import type { ExplorerSearch } from './search';

export const useActiveRelationshipFilters = ({
  contextCode,
  contextId,
  facets,
  search,
}: {
  contextCode: string;
  contextId?: string;
  facets: ExplorerRelationshipFacet[];
  search: ExplorerSearch;
}) => {
  const selectedFacets = facets.filter((facet) => facet.selectedIds.length > 0);
  const labelQueries = useQueries({
    queries: selectedFacets.map((facet) => ({
      queryKey: entityQueryKeys.relationshipTreeFacetChildren(
        search.blueprint ?? '',
        search.version,
        search.query,
        facet.sourceRelationship.code,
        facet.hierarchyField ?? '',
        contextCode,
        undefined,
        null,
        facet.selectedIds,
      ),
      queryFn: ({ signal }: { signal: AbortSignal }) =>
        getRelationshipTreeFacetChildren(
          {
            blueprint: {
              code: search.blueprint!,
              ...(search.version === undefined
                ? {}
                : { version: search.version }),
            },
            ...(search.query ? { query: search.query } : {}),
            source_relationship_field: facet.sourceRelationship.code,
            ...(facet.hierarchyField
              ? { hierarchy_field: facet.hierarchyField }
              : {}),
            context_id: contextId!,
            selected_target_ids: facet.selectedIds,
            cursor: null,
          },
          signal,
        ),
      enabled: Boolean(search.blueprint && contextId),
    })),
  });

  return selectedFacets.map((facet, index) => {
    const selectedItems = labelQueries[index]?.data?.selected_items;
    const labels =
      selectedItems?.length === facet.selectedIds.length &&
      selectedItems.every((item) => item.display)
        ? selectedItems.map((item) => item.display)
        : undefined;
    return {
      field: facet.sourceRelationship.code,
      selectedCount: facet.selectedIds.length,
      labels,
    };
  });
};

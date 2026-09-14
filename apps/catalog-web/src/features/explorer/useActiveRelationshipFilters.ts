import type { ExplorerRelationshipFacet } from './ExplorerFacetSidebar';

export const useActiveRelationshipFilters = ({
  facets,
}: {
  facets: ExplorerRelationshipFacet[];
}) =>
  facets
    .filter((facet) => facet.selectedIds.length > 0)
    .map((facet) => ({
      field: facet.sourceRelationship.code,
      selectedCount: facet.selectedIds.length,
    }));

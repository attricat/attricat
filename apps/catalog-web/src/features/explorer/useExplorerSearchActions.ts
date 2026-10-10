import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { defaultContextCode } from '../contexts/constants';
import {
  explorerResultsQueryKey,
  requestSort,
  submittedExplorerSearch,
  toggledSort,
  updateRelationshipFacets,
  withoutRelationshipPathScope,
  type RelationshipFilter,
} from './explorerSearchState';
import type { RelationshipFacetUpdate } from './relationshipFilterTypes';
import type { AttributeFilter, ExplorerSearch } from './search';

/** URL-backed Explorer search transitions. */
export const useExplorerSearchActions = (
  search: ExplorerSearch,
  {
    currentVersion,
    relationshipFilters,
  }: {
    currentVersion: number | undefined;
    relationshipFilters: RelationshipFilter[];
  },
) => {
  const navigate = useNavigate({ from: '/' });
  const queryClient = useQueryClient();
  // The record side panel stays open while the search changes.
  const go = (nextSearch: ExplorerSearch) =>
    void navigate({
      to: '/',
      search: ({ record }) => ({ ...nextSearch, record }),
    });

  const setAttributeFilters = (attributeFilters: AttributeFilter[]) =>
    go({
      ...search,
      attributeFilters: attributeFilters.length ? attributeFilters : undefined,
    });

  return {
    addAttributeFilter: (filter: AttributeFilter) =>
      go({
        ...search,
        attributeFilters: [...(search.attributeFilters ?? []), filter],
      }),
    updateAttributeFilter: (index: number, filter: AttributeFilter) => {
      const attributeFilters = [...(search.attributeFilters ?? [])];
      attributeFilters[index] = filter;
      go({ ...search, attributeFilters });
    },
    removeAttributeFilter: (index: number) =>
      setAttributeFilters(
        (search.attributeFilters ?? []).filter(
          (_, filterIndex) => filterIndex !== index,
        ),
      ),
    updateFacet: (field: string, updates: RelationshipFacetUpdate) =>
      go({
        ...search,
        relationshipFacets: updateRelationshipFacets(
          search.relationshipFacets,
          field,
          updates,
        ),
      }),
    changeContext: (context: string) => go({ ...search, context }),
    selectBlueprint: (blueprint: string) =>
      go({
        blueprint,
        sourceView: search.sourceView,
        query: search.query,
        relationshipFacets: undefined,
        attributeFilters: undefined,
      }),
    showAllVersions: () =>
      go({
        ...search,
        version: undefined,
        allVersions: true,
        ...withoutRelationshipPathScope(search),
      }),
    toggleSort: (field: string) =>
      go({ ...search, sort: toggledSort(search.sort, field) }),
    submit: (value: ExplorerSearch) => {
      const { keepsBlueprint, nextSearch, scopeChanged } =
        submittedExplorerSearch(search, value);
      // Resubmitting an unchanged search refreshes its first page.
      void queryClient.invalidateQueries({
        exact: true,
        queryKey: explorerResultsQueryKey(
          nextSearch,
          keepsBlueprint && !scopeChanged ? relationshipFilters : [],
          nextSearch.allVersions
            ? undefined
            : (nextSearch.version ?? currentVersion),
          requestSort(
            nextSearch.sort,
            nextSearch.context ?? defaultContextCode,
          ),
          nextSearch.context ?? defaultContextCode,
        ),
      });
      go(nextSearch);
    },
  };
};

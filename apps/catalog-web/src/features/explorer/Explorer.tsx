import {
  keepPreviousData,
  useInfiniteQuery,
  useQueries,
  useQuery,
} from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Box, Typography } from '@mui/material';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/query-keys';
import {
  getBlueprintByCode,
  getRelationshipTreeFacetChildren,
  listEntityBlueprints,
  searchEntities,
} from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import {
  ExplorerFacetSidebar,
  type ExplorerRelationshipFacet,
} from './ExplorerFacetSidebar';
import { ActiveExplorerFilters } from './ActiveExplorerFilters';
import { ExplorerResultsTable } from './ExplorerResultsTable';
import { ExplorerSearchForm } from './ExplorerSearchForm';
import type { AttributeFilter, ExplorerSearch } from './search';

const lastBlueprintStorageKey = 'catalog.explorer.last-blueprint';

const getLastBlueprint = () => {
  try {
    return sessionStorage.getItem(lastBlueprintStorageKey) || undefined;
  } catch {
    return undefined;
  }
};

export const Explorer = ({ search: urlSearch }: { search: ExplorerSearch }) => {
  const { t } = useTranslation();
  const search = {
    ...urlSearch,
    blueprint: urlSearch.blueprint ?? getLastBlueprint(),
  };
  const navigate = useNavigate({ from: '/' });

  useEffect(() => {
    if (urlSearch.blueprint)
      sessionStorage.setItem(lastBlueprintStorageKey, urlSearch.blueprint);
  }, [urlSearch.blueprint]);

  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const selectedBlueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(search.blueprint, search.version),
    queryFn: ({ signal }) =>
      getBlueprintByCode(search.blueprint!, search.version, signal),
    enabled: Boolean(search.blueprint),
  });
  const relationshipFields = (selectedBlueprint.data?.attributes ?? []).filter(
    (
      attribute,
    ): attribute is typeof attribute & { target_blueprint_code: string } =>
      attribute.value_type === 'relationship' &&
      typeof attribute.target_blueprint_code === 'string',
  );
  const facetContextCode = search.relationshipFacets?.[0]?.context ?? 'default';
  const explorerFacets: ExplorerRelationshipFacet[] = relationshipFields.map(
    (sourceRelationship) => {
      const saved = search.relationshipFacets?.find(
        (facet) => facet.field === sourceRelationship.code,
      );
      return {
        hierarchyField: saved?.hierarchy,
        selectedIds: saved?.selectedIds ?? [],
        sourceRelationship,
      };
    },
  );
  const relationshipTreeFacets = explorerFacets.flatMap((facet) => {
    const contextId = contexts.data?.find(
      (context) => context.code === facetContextCode,
    )?.id;
    if (!facet.selectedIds.length || !contextId) return [];
    return [
      {
        source_relationship_field: facet.sourceRelationship.code,
        ...(facet.hierarchyField
          ? { hierarchy_field: facet.hierarchyField }
          : {}),
        context_id: contextId,
        selected_target_ids: facet.selectedIds,
      },
    ];
  });
  const results = useInfiniteQuery({
    queryKey: entityQueryKeys.search(
      search.blueprint,
      search.version,
      search.query,
      relationshipTreeFacets,
      search.sort,
      search.attributeFilters,
    ),
    queryFn: ({ pageParam, signal }) =>
      searchEntities(
        search.blueprint!,
        search.version,
        search.query ?? '',
        pageParam,
        relationshipTreeFacets,
        signal,
        search.sort,
        pageParam === null,
        search.attributeFilters,
      ),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    enabled: Boolean(search.blueprint),
    placeholderData: keepPreviousData,
  });
  const resultPages = results.data?.pages ?? [];
  const resultItems = resultPages.flatMap((page) => page.items);
  const resultBlueprint = resultPages[0]?.blueprint;
  const blueprints = useQuery({
    queryKey: entityQueryKeys.blueprints(),
    queryFn: ({ signal }) => listEntityBlueprints(signal),
  });
  const updateFacetContext = (context: string) => {
    const current = search.relationshipFacets ?? [];
    void navigate({
      to: '/',
      search: {
        ...search,
        relationshipFacets: relationshipFields.map((field) => {
          const existing = current.find((facet) => facet.field === field.code);
          return {
            field: field.code,
            ...existing,
            context,
            selectedIds: [],
          };
        }),
      },
    });
  };
  const updateFacet = (
    field: string,
    updates: { hierarchy?: string; context?: string; selectedIds?: string[] },
  ) => {
    const current = search.relationshipFacets ?? [];
    const existing = current.find((facet) => facet.field === field);
    const nextFacet = { field, ...existing, ...updates };
    void navigate({
      to: '/',
      search: {
        ...search,
        relationshipFacets: [
          ...current.filter((facet) => facet.field !== field),
          nextFacet,
        ],
      },
    });
  };

  const selectedRelationshipFacets = explorerFacets.filter(
    (facet) => facet.selectedIds.length > 0,
  );
  const relationshipFilterLabelQueries = useQueries({
    queries: selectedRelationshipFacets.map((facet) => {
      const contextId = contexts.data?.find(
        (context) => context.code === facetContextCode,
      )?.id;
      return {
        queryKey: entityQueryKeys.relationshipTreeFacetChildren(
          search.blueprint ?? '',
          search.version,
          search.query,
          facet.sourceRelationship.code,
          facet.hierarchyField ?? '',
          facetContextCode,
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
      };
    }),
  });
  const activeRelationshipFilters = selectedRelationshipFacets.map(
    (facet, index) => {
      const selectedItems =
        relationshipFilterLabelQueries[index]?.data?.selected_items;
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
    },
  );
  const addAttributeFilter = (filter: AttributeFilter) => {
    void navigate({
      to: '/',
      search: {
        ...search,
        attributeFilters: [...(search.attributeFilters ?? []), filter],
      },
    });
  };
  const removeAttributeFilter = (index: number) => {
    const attributeFilters = (search.attributeFilters ?? []).filter(
      (_, filterIndex) => filterIndex !== index,
    );
    void navigate({
      to: '/',
      search: {
        ...search,
        attributeFilters: attributeFilters.length
          ? attributeFilters
          : undefined,
      },
    });
  };

  const lockedBlueprintName =
    selectedBlueprint.data?.blueprint.name ?? search.blueprint;
  const selectBlueprint = (blueprint: string) => {
    void navigate({
      to: '/',
      search: {
        blueprint,
        query: search.query,
        relationshipFacets: undefined,
        attributeFilters: undefined,
      },
    });
  };

  return (
    <Box sx={{ display: 'flex', minHeight: '100dvh' }}>
      <Box
        sx={{
          display: { xs: 'none', lg: 'block' },
          flexShrink: 0,
          width: 300,
        }}
      >
        <ExplorerFacetSidebar
          activeAttributeFilterCount={search.attributeFilters?.length ?? 0}
          attributes={selectedBlueprint.data?.attributes ?? []}
          blueprint={search.blueprint ?? ''}
          blueprints={blueprints.data ?? []}
          contextCode={facetContextCode}
          contexts={contexts.data ?? []}
          facets={explorerFacets}
          fullHeight
          onAddAttributeFilter={addAttributeFilter}
          onBlueprintChange={search.locked ? undefined : selectBlueprint}
          onContextChange={updateFacetContext}
          onUpdate={updateFacet}
          query={search.query}
          version={search.version}
        />
      </Box>
      <Box sx={{ flexGrow: 1, minWidth: 0 }}>
        <PageContainer>
          <PageHeader
            actions={
              <RouterButton
                search={
                  search.locked && search.blueprint
                    ? { blueprint: search.blueprint, locked: true }
                    : {}
                }
                to="/entities/new"
                variant="contained"
              >
                {search.locked && lockedBlueprintName
                  ? t('explorer.createBlueprint', {
                      blueprint: lockedBlueprintName,
                    })
                  : t('explorer.create')}
              </RouterButton>
            }
            description={
              search.locked && lockedBlueprintName
                ? t('explorer.blueprintDescription', {
                    blueprint: lockedBlueprintName,
                  })
                : t('explorer.description')
            }
            title={
              search.locked && lockedBlueprintName
                ? t('explorer.blueprintTitle', {
                    blueprint: lockedBlueprintName,
                  })
                : t('explorer.title')
            }
          />
          <ExplorerSearchForm
            blueprints={blueprints.data ?? []}
            lockedBlueprint
            onSubmit={(value) => {
              void navigate({
                to: '/',
                search: {
                  ...(value.blueprint === search.blueprint
                    ? search
                    : {
                        relationshipFacets: undefined,
                        attributeFilters: undefined,
                      }),
                  ...value,
                },
              });
            }}
            search={search}
          />
          <ActiveExplorerFilters
            attributeFilters={search.attributeFilters ?? []}
            onRemoveAttribute={removeAttributeFilter}
            onRemoveRelationship={(field) =>
              updateFacet(field, { selectedIds: [] })
            }
            relationshipFilters={activeRelationshipFilters}
          />
          {!search.blueprint && (
            <Typography sx={{ py: 3 }}>{t('explorer.start')}</Typography>
          )}
          {search.blueprint && results.isPending && (
            <Typography sx={{ py: 3 }}>{t('explorer.loading')}</Typography>
          )}
          {results.isError && (
            <Alert severity="error" sx={{ mt: 3 }}>
              {results.error.message}
            </Alert>
          )}
          {results.data && (
            <Box
              sx={{
                mt: 3,
              }}
            >
              <Box sx={{ display: { lg: 'none' }, mb: 3 }}>
                <ExplorerFacetSidebar
                  activeAttributeFilterCount={
                    search.attributeFilters?.length ?? 0
                  }
                  attributes={selectedBlueprint.data?.attributes ?? []}
                  blueprint={search.blueprint ?? ''}
                  blueprints={blueprints.data ?? []}
                  contextCode={facetContextCode}
                  contexts={contexts.data ?? []}
                  facets={explorerFacets}
                  onAddAttributeFilter={addAttributeFilter}
                  onBlueprintChange={
                    search.locked ? undefined : selectBlueprint
                  }
                  onContextChange={updateFacetContext}
                  onUpdate={updateFacet}
                  query={search.query}
                  version={search.version}
                />
              </Box>
              {resultBlueprint && (
                <ExplorerResultsTable
                  blueprint={resultBlueprint}
                  key={JSON.stringify([
                    search.blueprint,
                    search.version,
                    search.query,
                    relationshipTreeFacets,
                    search.sort,
                    search.attributeFilters,
                  ])}
                  hasNextPage={results.hasNextPage}
                  isFetchingNextPage={results.isFetchingNextPage}
                  items={resultItems}
                  totalCount={resultPages[0]?.total_count ?? null}
                  totalCountCapped={resultPages[0]?.total_count_capped ?? false}
                  onLoadMore={() => void results.fetchNextPage()}
                  onSortChange={(field) => {
                    const direction =
                      search.sort?.field === field &&
                      search.sort.direction === 'asc'
                        ? 'desc'
                        : 'asc';
                    void navigate({
                      to: '/',
                      search: { ...search, sort: { field, direction } },
                    });
                  }}
                  sort={search.sort}
                />
              )}
            </Box>
          )}
        </PageContainer>
      </Box>
    </Box>
  );
};

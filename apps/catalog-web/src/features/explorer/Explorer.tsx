import {
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Stack,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { useEffect, useMemo } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { useMobileExplorePanelTarget } from '../../components/mobile-navigation-panel-context';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { RouterButton } from '../../components/RouterLink';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/query-keys';
import { defaultContextCode } from '../contexts/constants';
import {
  getBlueprintByCode,
  listEntityBlueprints,
  searchEntities,
} from '../entities/api';
import { listBlueprintRevisions } from '../blueprints/api';
import { entityQueryKeys } from '../entities/query-keys';
import {
  ExplorerFacetSidebar,
  type ExplorerRelationshipFacet,
} from './ExplorerFacetSidebar';
import { ActiveExplorerFilters } from './ActiveExplorerFilters';
import { ExplorerResultsTable } from './ExplorerResultsTable';
import { ExplorerSearchForm } from './ExplorerSearchForm';
import type { AttributeFilter, ExplorerSearch } from './search';
import { useActiveRelationshipFilters } from './useActiveRelationshipFilters';
import { getLastBlueprint, setLastBlueprint } from './last-blueprint';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import { ApiRequestError } from '../../api/request';

export const Explorer = ({ search: urlSearch }: { search: ExplorerSearch }) => {
  const { t } = useTranslation();
  const theme = useTheme();
  const isWideDesktop = useMediaQuery(theme.breakpoints.up('lg'));
  const mobileExplorePanelTarget = useMobileExplorePanelTarget();
  const search = useMemo(
    () => ({
      ...urlSearch,
      blueprint: urlSearch.blueprint ?? getLastBlueprint(),
    }),
    [urlSearch],
  );
  const navigate = useNavigate({ from: '/' });
  const queryClient = useQueryClient();

  useEffect(() => {
    if (urlSearch.blueprint) setLastBlueprint(urlSearch.blueprint);
  }, [urlSearch.blueprint]);

  const blueprints = useQuery({
    queryKey: entityQueryKeys.blueprints(),
    queryFn: ({ signal }) => listEntityBlueprints(signal),
  });
  const currentBlueprint = blueprints.data?.find(
    (blueprint) => blueprint.code === search.blueprint,
  );
  const effectiveVersion = search.allVersions
    ? undefined
    : (search.version ?? currentBlueprint?.version);
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const selectedBlueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(
      search.blueprint,
      effectiveVersion,
    ),
    queryFn: ({ signal }) =>
      getBlueprintByCode(search.blueprint!, effectiveVersion, signal),
    enabled: Boolean(
      search.blueprint &&
      (search.allVersions || effectiveVersion !== undefined),
    ),
  });
  const revisions = useQuery({
    queryKey: entityQueryKeys.blueprintRevisions(currentBlueprint?.id),
    queryFn: async ({ signal }) =>
      (await listBlueprintRevisions(currentBlueprint!.id!, signal)).filter(
        (revision) => revision.status === 'published',
      ),
    enabled: Boolean(currentBlueprint?.id),
  });
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });

  useEffect(() => {
    if (
      search.version !== undefined &&
      revisions.data &&
      !revisions.data.some((revision) => revision.version === search.version)
    ) {
      void navigate({
        to: '/',
        search: {
          ...search,
          version: undefined,
          relationshipFacets: undefined,
          attributeFilters: undefined,
          sort: undefined,
        },
        replace: true,
      });
    }
  }, [navigate, revisions.data, search]);

  const relationshipFields = (selectedBlueprint.data?.attributes ?? []).filter(
    (
      attribute,
    ): attribute is typeof attribute & { target_blueprint_code: string } =>
      attribute.value_type === 'relationship' &&
      typeof attribute.target_blueprint_code === 'string',
  );
  const facetContextCode =
    search.context ??
    search.relationshipFacets?.[0]?.context ??
    defaultContextCode;
  const facetContextId = contexts.data?.find(
    (context) => context.code === facetContextCode,
  )?.id;
  const relationshipPathFields = [
    ...relationshipFields,
    ...(search.relationshipFacets ?? [])
      .filter(
        (facet) =>
          facet.targetBlueprint &&
          !relationshipFields.some((field) => field.code === facet.field),
      )
      .map(
        (facet) =>
          ({
            cardinality: 'many',
            code: facet.field,
            target_blueprint_code: facet.targetBlueprint,
            value_type: 'relationship',
          }) as (typeof relationshipFields)[number],
      ),
  ];
  const explorerFacets: ExplorerRelationshipFacet[] = relationshipPathFields.map(
    (sourceRelationship) => {
      const saved = search.relationshipFacets?.find(
        (facet) => facet.field === sourceRelationship.code,
      );
      return {
        selectedIds: saved?.selectedIds ?? [],
        sourceRelationship,
      };
    },
  );
  const visibleExplorerFacets = explorerFacets.filter(
    (facet) =>
      facet.selectedIds.length > 0 ||
      search.relationshipFacets?.some(
        (saved) => saved.field === facet.sourceRelationship.code,
      ),
  );
  const relationshipFilters = (search.relationshipFacets ?? []).flatMap(
    (facet) =>
      facet.selectedIds?.length
        ? [{ field: facet.field, selected_target_ids: facet.selectedIds }]
        : [],
  );
  const results = useInfiniteQuery({
    queryKey: entityQueryKeys.search(
      search.blueprint,
      effectiveVersion,
      search.query,
      search.sort,
      search.attributeFilters,
      Boolean(search.allVersions),
      relationshipFilters,
    ),
    queryFn: ({ pageParam, signal }) =>
      searchEntities(
        search.blueprint!,
        effectiveVersion,
        search.query ?? '',
        pageParam,
        signal,
        search.sort,
        pageParam === null,
        search.attributeFilters,
        relationshipFilters,
      ),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    enabled: Boolean(
      search.blueprint &&
      (search.allVersions || effectiveVersion !== undefined),
    ),
    placeholderData: keepPreviousData,
  });
  useEffect(() => {
    if (
      results.error instanceof ApiRequestError &&
      results.error.code ===
        'relationship_path_sort_requires_single_result_version' &&
      search.sort?.field.includes('.')
    ) {
      void navigate({
        to: '/',
        search: { ...search, sort: undefined },
        replace: true,
      });
    }
  }, [navigate, results.error, search]);
  const resultPages = results.data?.pages ?? [];
  const resultItems = resultPages.flatMap((page) => page.items);
  const resultBlueprint = resultPages[0]?.blueprint;
  const updateFacetContext = (context: string) => {
    void navigate({ to: '/', search: { ...search, context } });
  };
  const updateFacet = (
    field: string,
    updates: { selectedIds?: string[]; targetBlueprint?: string },
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

  const activeRelationshipFilters = useActiveRelationshipFilters({
    facets: explorerFacets,
  });
  const addAttributeFilter = (filter: AttributeFilter) => {
    void navigate({
      to: '/',
      search: {
        ...search,
        attributeFilters: [...(search.attributeFilters ?? []), filter],
      },
    });
  };
  const updateAttributeFilter = (index: number, filter: AttributeFilter) => {
    const attributeFilters = [...(search.attributeFilters ?? [])];
    attributeFilters[index] = filter;
    void navigate({
      to: '/',
      search: { ...search, attributeFilters },
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
  const facetSidebarProps = {
    attributeFilters: search.attributeFilters ?? [],
    attributes: selectedBlueprint.data?.attributes ?? [],
    blueprint: search.blueprint ?? '',
    blueprints: blueprints.data ?? [],
    contextCode: facetContextCode,
    contexts: contexts.data ?? [],
    facets: visibleExplorerFacets,
    onAddAttributeFilter: addAttributeFilter,
    onAddRelationshipFilter: (attribute) =>
      updateFacet(attribute.code, {
        selectedIds: [],
        targetBlueprint: attribute.target_blueprint_code,
      }),
    relationshipAttributes: relationshipFields,
    onBlueprintChange: search.locked ? undefined : selectBlueprint,
    onContextChange: updateFacetContext,
    onRemoveAttributeFilter: removeAttributeFilter,
    onUpdate: updateFacet,
    onUpdateAttributeFilter: updateAttributeFilter,
    pathAttributes: selectedBlueprint.data?.table_path_attributes,
    query: search.query,
    version: effectiveVersion,
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
        <ExplorerFacetSidebar {...facetSidebarProps} fullHeight />
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
            currentVersion={currentBlueprint?.version}
            revisions={revisions.data}
            revisionsError={revisions.error?.message}
            revisionsLoading={revisions.isPending}
            onRetryRevisions={() => void revisions.refetch()}
            lockedBlueprint
            onSubmit={(value) => {
              const keepsBlueprint = value.blueprint === search.blueprint;
              const scopeChanged =
                value.version !== search.version ||
                Boolean(value.allVersions) !== Boolean(search.allVersions);
              const nextSearch: ExplorerSearch = {
                ...(keepsBlueprint
                  ? search
                  : {
                      relationshipFacets: undefined,
                      attributeFilters: undefined,
                    }),
                version: undefined,
                allVersions: undefined,
                ...value,
                ...(scopeChanged
                  ? {
                      relationshipFacets: undefined,
                      attributeFilters: search.attributeFilters?.filter(
                        (filter) => !filter.field.includes('.'),
                      ),
                      sort: search.sort?.field.includes('.')
                        ? undefined
                        : search.sort,
                    }
                  : {}),
              };
              void queryClient.invalidateQueries({
                exact: true,
                queryKey: entityQueryKeys.search(
                  nextSearch.blueprint,
                  nextSearch.allVersions
                    ? undefined
                    : (nextSearch.version ?? currentBlueprint?.version),
                  nextSearch.query,
                  nextSearch.sort,
                  nextSearch.attributeFilters,
                  Boolean(nextSearch.allVersions),
                  keepsBlueprint && !scopeChanged ? relationshipFilters : [],
                ),
              });
              void navigate({ to: '/', search: nextSearch });
            }}
            search={search}
          />
          <ActiveExplorerFilters
            filters={[
              ...(search.attributeFilters ?? []).map((filter, index) => ({
                filter,
                index,
                kind: 'attribute' as const,
              })),
              ...activeRelationshipFilters.map((filter) => ({
                ...filter,
                kind: 'relationship' as const,
              })),
            ]}
            onRemoveAttribute={removeAttributeFilter}
            onRemoveRelationship={(field) =>
              updateFacet(field, { selectedIds: [] })
            }
          />
          {!search.blueprint && (
            <Typography sx={{ py: 3 }}>{t('explorer.start')}</Typography>
          )}
          {search.blueprint && results.isPending && !results.data && (
            <Box
              sx={{
                alignItems: 'center',
                display: 'flex',
                justifyContent: 'center',
                minHeight: '50vh',
              }}
            >
              <CircularProgress
                aria-label={t('explorer.loading')}
                enableTrackSlot
                size={80}
              />
            </Box>
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
              {!search.allVersions &&
                effectiveVersion === currentBlueprint?.version &&
                (resultPages[0]?.hidden_outdated_count ?? 0) > 0 && (
                  <Alert
                    action={
                      <Stack direction="row" spacing={1}>
                        <Button
                          onClick={() =>
                            void navigate({
                              to: '/',
                              search: {
                                ...search,
                                version: undefined,
                                allVersions: true,
                                relationshipFacets: undefined,
                                attributeFilters:
                                  search.attributeFilters?.filter(
                                    (filter) => !filter.field.includes('.'),
                                  ),
                                sort: search.sort?.field.includes('.')
                                  ? undefined
                                  : search.sort,
                              },
                            })
                          }
                          size="small"
                        >
                          {t('explorer.showAllVersions')}
                        </Button>
                        {session.data?.capabilities?.data_health_read && (
                          <RouterButton
                            size="small"
                            to="/manage/data-health"
                            variant="text"
                          >
                            {t('explorer.reviewMigrations')}
                          </RouterButton>
                        )}
                      </Stack>
                    }
                    severity="info"
                    sx={{ mb: 2 }}
                  >
                    {resultPages[0]?.hidden_outdated_count_capped
                      ? t('explorer.hiddenOutdatedCapped', {
                          count: resultPages[0].hidden_outdated_count,
                        })
                      : t('explorer.hiddenOutdated', {
                          count: resultPages[0]?.hidden_outdated_count,
                        })}
                  </Alert>
                )}
              {resultBlueprint && (
                <ExplorerResultsTable
                  blueprint={resultBlueprint}
                  key={JSON.stringify([
                    search.blueprint,
                    effectiveVersion,
                    search.allVersions,
                    search.query,
                    search.sort,
                    search.attributeFilters,
                  ])}
                  canPublish={
                    session.data?.capabilities?.entities_publish === true
                  }
                  hasNextPage={results.hasNextPage}
                  isFetching={results.isFetching}
                  isFetchingNextPage={results.isFetchingNextPage}
                  items={resultItems}
                  publicationContextCode={facetContextCode}
                  publicationContextId={facetContextId}
                  totalCount={resultPages[0]?.total_count ?? null}
                  totalCountCapped={resultPages[0]?.total_count_capped ?? false}
                  onLoadMore={() => void results.fetchNextPage()}
                  relationshipSortAvailable={
                    !search.allVersions ||
                    resultPages[0]?.result_version_scope.kind === 'single'
                  }
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
      {mobileExplorePanelTarget &&
        !isWideDesktop &&
        createPortal(
          <ExplorerFacetSidebar {...facetSidebarProps} />,
          mobileExplorePanelTarget,
        )}
    </Box>
  );
};

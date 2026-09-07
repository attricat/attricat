import {
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
} from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Box, Button, Typography } from '@mui/material';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  getBlueprintByCode,
  listContexts,
  listEntityBlueprints,
  searchEntities,
} from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import {
  ExplorerFacetSidebar,
  type ExplorerRelationshipFacet,
} from './ExplorerFacetSidebar';
import { ExplorerResultsTable } from './ExplorerResultsTable';
import { ExplorerSearchForm } from './ExplorerSearchForm';
import type { ExplorerSearch } from './search';

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
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const selectedBlueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(search.blueprint, search.version),
    queryFn: () => getBlueprintByCode(search.blueprint!, search.version),
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
    ),
    queryFn: ({ pageParam }) =>
      searchEntities(
        search.blueprint!,
        search.version,
        search.query ?? '',
        pageParam,
        relationshipTreeFacets,
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
    queryFn: listEntityBlueprints,
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

  return (
    <PageContainer>
      <PageHeader
        actions={
          <Button
            component="a"
            href={
              search.locked && search.blueprint
                ? `/entities/new?blueprint=${encodeURIComponent(search.blueprint)}&locked=true`
                : '/entities/new'
            }
            variant="contained"
          >
            {t('explorer.create')}
          </Button>
        }
        description={t('explorer.description')}
        title={t('explorer.title')}
      />
      <ExplorerSearchForm
        blueprints={blueprints.data ?? []}
        lockedBlueprint={search.locked}
        onSubmit={(value) => {
          void navigate({
            to: '/',
            search: {
              ...(value.blueprint === search.blueprint
                ? search
                : { relationshipFacets: undefined }),
              ...value,
            },
          });
        }}
        search={search}
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
            display: 'grid',
            gap: 3,
            gridTemplateColumns: {
              xs: '1fr',
              lg: 'minmax(240px, 300px) minmax(0, 1fr)',
            },
            mt: 3,
          }}
        >
          <ExplorerFacetSidebar
            blueprint={search.blueprint ?? ''}
            contextCode={facetContextCode}
            contexts={contexts.data ?? []}
            facets={explorerFacets}
            onContextChange={updateFacetContext}
            onUpdate={updateFacet}
            query={search.query}
            version={search.version}
          />
          {resultBlueprint && (
            <ExplorerResultsTable
              blueprint={resultBlueprint}
              key={JSON.stringify([
                search.blueprint,
                search.version,
                search.query,
                relationshipTreeFacets,
              ])}
              hasNextPage={results.hasNextPage}
              isFetchingNextPage={results.isFetchingNextPage}
              items={resultItems}
              onLoadMore={() => void results.fetchNextPage()}
            />
          )}
        </Box>
      )}
    </PageContainer>
  );
};

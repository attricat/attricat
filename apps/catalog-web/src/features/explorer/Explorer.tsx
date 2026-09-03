import {
  keepPreviousData,
  useInfiniteQuery,
  useQueries,
  useQuery,
} from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { Alert, Box, Button, Typography } from '@mui/material';
import { useEffect } from 'react';
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
  const targets = useQueries({
    queries: relationshipFields.map((field) => ({
      queryKey: entityQueryKeys.blueprintByCode(
        field.target_blueprint_code,
        undefined,
      ),
      queryFn: () => getBlueprintByCode(field.target_blueprint_code!),
    })),
  });
  const contextCodeByField = new Map(
    (search.relationshipFacets ?? []).map((facet) => [
      facet.field,
      facet.context ?? 'default',
    ]),
  );
  const explorerFacets: ExplorerRelationshipFacet[] = relationshipFields.map(
    (sourceRelationship, index) => {
      const saved = search.relationshipFacets?.find(
        (facet) => facet.field === sourceRelationship.code,
      );
      const targetBlueprint = targets[index]?.data;
      const hierarchyFields = (targetBlueprint?.attributes ?? [])
        .filter(
          (attribute) =>
            attribute.value_type === 'relationship' &&
            attribute.target_blueprint_code ===
              sourceRelationship.target_blueprint_code,
        )
        .map((attribute) => attribute.code);
      return {
        contextCode: saved?.context ?? 'default',
        sourceRelationship,
        targetBlueprint,
        hierarchyFields,
        hierarchyField: hierarchyFields.includes(saved?.hierarchy ?? '')
          ? saved?.hierarchy
          : hierarchyFields[0],
        selectedIds: saved?.selectedIds ?? [],
      };
    },
  );
  const relationshipTreeFacets = explorerFacets.flatMap((facet) => {
    const contextId = contexts.data?.find(
      (context) =>
        context.code === contextCodeByField.get(facet.sourceRelationship.code),
    )?.id;
    if (!facet.targetBlueprint || !contextId) return [];
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
          <Button component={Link} to="/entities/new" variant="contained">
            Create entity
          </Button>
        }
        description="Search and browse your catalog."
        title="Entity explorer"
      />
      <ExplorerSearchForm
        blueprints={blueprints.data ?? []}
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
        <Typography sx={{ py: 3 }}>
          Enter a blueprint code to start exploring.
        </Typography>
      )}
      {search.blueprint && results.isPending && (
        <Typography sx={{ py: 3 }}>Loading entities...</Typography>
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
            contexts={contexts.data ?? []}
            facets={explorerFacets}
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

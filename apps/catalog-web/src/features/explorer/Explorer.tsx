import {
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
} from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { Alert, Box, Button, Typography } from '@mui/material';
import { useEffect } from 'react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  listEntityBlueprints,
  getBlueprintByCode,
  listContexts,
  searchEntities,
} from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import { ExplorerFacetSidebar } from './ExplorerFacetSidebar';
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
    // A URL selection is shareable and therefore takes precedence over the
    // per-tab fallback.
    blueprint: urlSearch.blueprint ?? getLastBlueprint(),
  };
  const navigate = useNavigate({ from: '/' });

  useEffect(() => {
    if (!urlSearch.blueprint) return;

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
  const sourceRelationship = selectedBlueprint.data?.attributes.find(
    (attribute) =>
      attribute.code === search.facetField &&
      attribute.value_type === 'relationship' &&
      attribute.target_blueprint_code,
  );
  const targetBlueprint = useQuery({
    queryKey: entityQueryKeys.blueprintByCode(
      sourceRelationship?.target_blueprint_code ?? undefined,
      undefined,
    ),
    queryFn: () =>
      getBlueprintByCode(sourceRelationship!.target_blueprint_code!),
    enabled: Boolean(sourceRelationship?.target_blueprint_code),
  });
  const hierarchyFields = (targetBlueprint.data?.attributes ?? [])
    .filter(
      (attribute) =>
        attribute.value_type === 'relationship' &&
        attribute.target_blueprint_code ===
          sourceRelationship?.target_blueprint_code,
    )
    .map((attribute) => attribute.code);
  const hierarchyField = hierarchyFields.includes(search.facetHierarchy ?? '')
    ? search.facetHierarchy
    : hierarchyFields[0];
  const contextCode = search.facetContext ?? 'default';
  const contextId = contexts.data?.find(
    (context) => context.code === contextCode,
  )?.id;
  const relationshipTreeFacet =
    sourceRelationship && contextId
      ? {
          source_relationship_field: sourceRelationship.code,
          ...(hierarchyField ? { hierarchy_field: hierarchyField } : {}),
          context_id: contextId,
          selected_target_ids: search.categories ?? [],
        }
      : undefined;
  const results = useInfiniteQuery({
    queryKey: entityQueryKeys.search(
      search.blueprint,
      search.version,
      search.query,
      relationshipTreeFacet,
    ),
    queryFn: ({ pageParam }) =>
      searchEntities(
        search.blueprint!,
        search.version,
        search.query ?? '',
        pageParam,
        relationshipTreeFacet,
      ),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    enabled: Boolean(search.blueprint),
    // Facet selections change this query's key. Preserve the current explorer
    // while the filtered result page is fetched instead of replacing it with a
    // full-page loading state.
    placeholderData: keepPreviousData,
  });
  const resultPages = results.data?.pages ?? [];
  const resultItems = resultPages.flatMap((page) => page.items);
  const resultBlueprint = resultPages[0]?.blueprint;
  const blueprints = useQuery({
    queryKey: entityQueryKeys.blueprints(),
    queryFn: listEntityBlueprints,
  });
  const relationshipFields = (selectedBlueprint.data?.attributes ?? []).filter(
    (attribute) =>
      attribute.value_type === 'relationship' &&
      attribute.target_blueprint_code,
  );
  const updateFacet = (updates: Partial<ExplorerSearch>) => {
    void navigate({
      to: '/',
      search: { ...search, ...updates },
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
                : {
                    facetField: undefined,
                    facetHierarchy: undefined,
                    facetContext: undefined,
                    categories: undefined,
                  }),
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
            contextCode={contextCode}
            contexts={contexts.data ?? []}
            hierarchyField={hierarchyField}
            hierarchyFields={hierarchyFields}
            isTargetBlueprintPending={targetBlueprint.isPending}
            onUpdate={updateFacet}
            query={search.query}
            relationshipFields={relationshipFields}
            searchFacetField={search.facetField}
            selectedIds={search.categories ?? []}
            sourceRelationship={sourceRelationship}
            version={search.version}
          />
          {resultBlueprint && (
            <ExplorerResultsTable
              blueprint={resultBlueprint}
              key={JSON.stringify([
                search.blueprint,
                search.version,
                search.query,
                relationshipTreeFacet,
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

import {
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
} from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';
import { ApiRequestError } from '../../api/request';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { listBlueprintRevisions } from '../blueprints/api';
import { listContexts } from '../contexts/api';
import { defaultContextCode } from '../contexts/constants';
import { contextQueryKeys } from '../contexts/queryKeys';
import {
  getBlueprintByCode,
  listEntityBlueprints,
  searchEntities,
} from '../entities/api';
import { isHiddenByDefault } from '../entities/attributeVisibility';
import { contextAncestorCodes } from '../entities/previewContext';
import { entityQueryKeys } from '../entities/queryKeys';
import { listPublicationChannels } from '../exports/api';
import { exportQueryKeys } from '../exports/queryKeys';
import {
  explorerVisibilityScope,
  publishedRevisionStatus,
  relationshipPathSortErrorCode,
} from './constants';
import {
  explorerResultsQueryKey,
  relationshipFiltersFromSearch,
  requestSort,
} from './explorerSearchState';
import { isRelationshipPath } from './explorerTableColumns';
import { isRelationshipFilterAttribute } from './relationshipFilterTypes';
import type { ExplorerSearch } from './search';

/**
 * Loads everything the Explorer page needs for the current search and keeps
 * the URL consistent with server-side constraints.
 */
export const useExplorerData = (search: ExplorerSearch) => {
  const navigate = useNavigate({ from: '/' });
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
  const versionResolved = search.allVersions || effectiveVersion !== undefined;
  const blueprintMissing = Boolean(
    search.blueprint && blueprints.isSuccess && !currentBlueprint,
  );
  const canSearch = Boolean(
    search.blueprint && !blueprintMissing && versionResolved,
  );
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
      getBlueprintByCode(search.blueprint ?? '', effectiveVersion, signal),
    enabled: Boolean(search.blueprint && versionResolved),
  });
  const revisions = useQuery({
    queryKey: entityQueryKeys.blueprintRevisions(currentBlueprint?.id),
    queryFn: async ({ signal }) =>
      (await listBlueprintRevisions(currentBlueprint?.id ?? '', signal)).filter(
        (revision) => revision.status === publishedRevisionStatus,
      ),
    enabled: Boolean(currentBlueprint?.id),
  });
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const channels = useQuery({
    queryKey: exportQueryKeys.channels(),
    queryFn: listPublicationChannels,
  });

  useEffect(() => {
    if (
      search.version !== undefined &&
      revisions.data &&
      !revisions.data.some((revision) => revision.version === search.version)
    ) {
      // Corrections keep the entity panel open, like other search changes.
      void navigate({
        to: '/',
        search: ({ entity }) => ({
          ...search,
          version: undefined,
          relationshipFacets: undefined,
          attributeFilters: undefined,
          sort: undefined,
          entity,
        }),
        replace: true,
      });
    }
  }, [navigate, revisions.data, search]);

  const relationshipFields = (selectedBlueprint.data?.attributes ?? [])
    .filter(isRelationshipFilterAttribute)
    .filter(
      (attribute) => !isHiddenByDefault(attribute, explorerVisibilityScope),
    );
  const contextCode = search.context ?? defaultContextCode;
  const contextId = contexts.data?.find(
    (context) => context.code === contextCode,
  )?.id;
  const contextCodes = contextAncestorCodes(contexts.data ?? [], contextCode);
  const publicationSortAvailable =
    channels.data?.some(
      (channel) => channel.context_code === contextCode && channel.enabled,
    ) ?? false;
  const sort = requestSort(search.sort, contextCode);
  const relationshipFilters = relationshipFiltersFromSearch(search);
  const results = useInfiniteQuery({
    queryKey: explorerResultsQueryKey(
      search,
      relationshipFilters,
      effectiveVersion,
      sort,
      contextCode,
    ),
    queryFn: ({ pageParam, signal }) =>
      searchEntities({
        blueprint: search.blueprint ?? '',
        contextCode,
        cursor: pageParam,
        filters: search.attributeFilters,
        includeTotal: pageParam === null,
        query: search.query,
        relationshipFilters,
        signal,
        sort,
        version: effectiveVersion,
      }),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    enabled: canSearch,
    placeholderData: keepPreviousData,
  });

  useEffect(() => {
    if (
      results.error instanceof ApiRequestError &&
      results.error.code === relationshipPathSortErrorCode &&
      search.sort &&
      isRelationshipPath(search.sort.field)
    ) {
      void navigate({
        to: '/',
        search: ({ entity }) => ({ ...search, sort: undefined, entity }),
        replace: true,
      });
    }
  }, [navigate, results.error, search]);

  return {
    blueprintMissing,
    blueprints,
    canSearch,
    contextCode,
    contextCodes,
    contextId,
    contexts,
    currentBlueprint,
    effectiveVersion,
    publicationSortAvailable,
    relationshipFields,
    relationshipFilters,
    results,
    revisions,
    selectedBlueprint,
    session,
  };
};

export type ExplorerData = ReturnType<typeof useExplorerData>;

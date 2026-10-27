import { revisionStatuses } from '../../api/revisionStatus';
import {
  keepPreviousData,
  useInfiniteQuery,
  useQuery,
} from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useMemo } from 'react';
import { ApiRequestError } from '../../api/request';
import { type Blueprint } from '../blueprints/api';
import { defaultContextCode } from '../contexts/constants';
import { searchRecordTablePage } from '../records/api';
import { isHiddenByDefault } from '../records/attributeVisibility';
import { contextAncestorCodes } from '../records/previewContext';
import {
  explorerVisibilityScope,
  relationshipPathSortErrorCode,
} from './constants';
import {
  explorerResultsQueryKey,
  relationshipFiltersFromSearch,
  relationshipTreeFacetsRequest,
  relationshipTreeSelectionsFromSearch,
  exportStatusRequest,
  requestSort,
} from './explorerSearchState';
import { isRelationshipPath } from './explorerTableColumns';
import { isRelationshipFilterAttribute } from './relationshipFilterTypes';
import type { ExplorerSearch } from './search';
import { sessionOptions } from '../auth/queryOptions';
import {
  blueprintByCodeOptions,
  recordBlueprintsOptions,
} from '../records/queryOptions';
import { contextListOptions } from '../contexts/queryOptions';
import { publicationChannelsOptions } from '../exports/queryOptions';
import { blueprintRevisionListOptions } from '../blueprints/queryOptions';

const publishedRevisions = (revisions: Blueprint[]) =>
  revisions.filter(
    (revision) => revision.status === revisionStatuses.published,
  );

/**
 * Loads everything the Explorer page needs for the current search and keeps
 * the URL consistent with server-side constraints.
 */
export const useExplorerData = (search: ExplorerSearch) => {
  const navigate = useNavigate({ from: '/' });
  const blueprints = useQuery(recordBlueprintsOptions());
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
  const contexts = useQuery(contextListOptions());
  const selectedBlueprint = useQuery({
    ...blueprintByCodeOptions(search.blueprint ?? '', effectiveVersion),
    enabled: Boolean(search.blueprint && versionResolved),
  });
  const revisions = useQuery({
    ...blueprintRevisionListOptions(currentBlueprint?.id),
    // The blueprint pages share this cache entry and need every revision.
    select: publishedRevisions,
  });
  const session = useQuery(sessionOptions());
  const channels = useQuery(publicationChannelsOptions());

  useEffect(() => {
    if (
      search.version !== undefined &&
      revisions.data &&
      !revisions.data.some((revision) => revision.version === search.version)
    ) {
      // Corrections keep the record panel open, like other search changes.
      void navigate({
        to: '/',
        search: ({ record }) => ({
          ...search,
          version: undefined,
          relationshipFacets: undefined,
          attributeFilters: undefined,
          sort: undefined,
          record,
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
  const contextCodes = useMemo(
    () => contextAncestorCodes(contexts.data ?? [], contextCode),
    [contexts.data, contextCode],
  );
  const publicationSortAvailable =
    channels.data?.some(
      (channel) => channel.context_code === contextCode && channel.enabled,
    ) ?? false;
  const sort = requestSort(search.sort, contextCode);
  const relationshipFilters = relationshipFiltersFromSearch(search);
  const relationshipTreeSelections =
    relationshipTreeSelectionsFromSearch(search);
  const results = useInfiniteQuery({
    queryKey: explorerResultsQueryKey(
      search,
      relationshipFilters,
      effectiveVersion,
      sort,
      contextCode,
    ),
    queryFn: ({ pageParam, signal }) =>
      searchRecordTablePage({
        blueprint: search.blueprint ?? '',
        contextCode,
        cursor: pageParam,
        filters: search.attributeFilters,
        includeTotal: pageParam === null,
        query: search.query,
        relationshipFilters,
        relationshipTreeFacets: relationshipTreeFacetsRequest(
          relationshipTreeSelections,
          contextId ?? '',
        ),
        signal,
        sort,
        version: effectiveVersion,
        exportStatus: exportStatusRequest(search),
        systemTags: search.systemTags,
      }),
    initialPageParam: null as string | null,
    getNextPageParam: (page) => page.next_cursor,
    // Hierarchy selections resolve their edges in the selected context.
    enabled:
      canSearch &&
      (relationshipTreeSelections.length === 0 || contextId !== undefined),
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
        search: ({ record }) => ({ ...search, sort: undefined, record }),
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
    relationshipTreeSelections,
    results,
    revisions,
    selectedBlueprint,
    session,
  };
};

export type ExplorerData = ReturnType<typeof useExplorerData>;

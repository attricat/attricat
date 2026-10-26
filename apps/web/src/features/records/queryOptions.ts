import {
  infiniteQueryOptions,
  queryOptions,
  skipToken,
} from '@tanstack/react-query';
import {
  getBlueprintByCode,
  getRecordApprovals,
  getRecordForm,
  getRecordLabels,
  getRecordPublicationReadiness,
  getRecordPublications,
  getRecordRetentionHolds,
  getResolvedRecordPreview,
  getStatusTransitions,
  listRecordBlueprints,
} from './api';
import { recordQueryKeys } from './queryKeys';
import {
  createdAtCursor,
  nextKeysetCursor,
  type KeysetCursor,
} from '../../api/keysetCursor';

// Revisited virtual rows reuse their result; mutations explicitly invalidate it.
const publicationStaleTimeMs = 60_000;

// Record data loaded ahead, such as a preview whose link is pointed at or a
// side panel's data started with the record, is not fetched again when the
// component showing it mounts; edits invalidate it.
export const readAheadStaleTimeMs = 10_000;

/**
 * A record's display labels by context, built from its blueprint's
 * `dropdown_option` view.
 */
export const recordLabelOptions = (recordId: string) =>
  queryOptions({
    queryKey: recordQueryKeys.labels([recordId]),
    queryFn: ({ signal }) => getRecordLabels([recordId], signal),
  });

/** A record's values as seen from a context. */
export const resolvedRecordPreviewOptions = (
  recordId: string,
  /** The context the values are seen from; waits while it is unknown. */
  contextId: string | null,
) =>
  queryOptions({
    queryKey: recordQueryKeys.resolvedPreview(recordId, contextId ?? undefined),
    queryFn: contextId
      ? () => getResolvedRecordPreview(recordId, contextId)
      : skipToken,
    staleTime: readAheadStaleTimeMs,
  });

/**
 * The record's values and blueprint as an editor needs them. The form carries
 * the revision its values belong to, which previews then read from the cache
 * instead of loading it after the record.
 */
export const recordFormOptions = (recordId: string) =>
  queryOptions({
    queryKey: recordQueryKeys.form(recordId),
    queryFn: async ({ client, signal }) => {
      const form = await getRecordForm(recordId, signal);
      const { id, version } = form.blueprint.blueprint;
      client.setQueryData(
        recordQueryKeys.blueprintRevision(id, version),
        (cached) => cached ?? form.blueprint,
      );
      return form;
    },
  });

export const recordPublicationOptions = (recordId: string) =>
  queryOptions({
    queryKey: recordQueryKeys.publication(recordId),
    queryFn: ({ signal }) => getRecordPublications(recordId, signal),
    staleTime: publicationStaleTimeMs,
  });

/** Readiness is evaluated live, so it is refetched whenever it is observed. */
export const recordPublicationReadinessOptions = (recordId: string) =>
  queryOptions({
    queryKey: recordQueryKeys.publicationReadiness(recordId),
    queryFn: ({ signal }) => getRecordPublicationReadiness(recordId, signal),
  });

export const recordApprovalsOptions = (recordId: string) =>
  infiniteQueryOptions({
    queryKey: recordQueryKeys.approvals(recordId),
    queryFn: ({ pageParam, signal }) =>
      getRecordApprovals(recordId, pageParam, signal),
    initialPageParam: undefined as KeysetCursor | undefined,
    getNextPageParam: (page) =>
      nextKeysetCursor(page, (approval) => ({
        before_time: approval.approved_at,
        before_id: approval.id,
      })),
    staleTime: readAheadStaleTimeMs,
  });

export const recordRetentionHoldsOptions = (recordId: string) =>
  infiniteQueryOptions({
    queryKey: recordQueryKeys.retentionHolds(recordId),
    queryFn: ({ pageParam, signal }) =>
      getRecordRetentionHolds(recordId, pageParam, signal),
    initialPageParam: undefined as KeysetCursor | undefined,
    getNextPageParam: (page) => nextKeysetCursor(page, createdAtCursor),
    staleTime: readAheadStaleTimeMs,
  });

/** The caller's access to declared status edges from the saved status. */
export const recordStatusTransitionsOptions = (
  recordId: string,
  contextId: string | null,
) =>
  queryOptions({
    queryKey: recordQueryKeys.statusTransitions(recordId, contextId),
    queryFn: ({ signal }) => getStatusTransitions(recordId, contextId, signal),
  });

/** A blueprint with its attributes by code, at its current version unless one is given. */
export const blueprintByCodeOptions = (code: string, version?: number) =>
  queryOptions({
    queryKey: recordQueryKeys.blueprintByCode(code, version),
    queryFn: ({ signal }) => getBlueprintByCode(code, version, signal),
  });

/** The blueprints records can be created from. */
export const recordBlueprintsOptions = () =>
  queryOptions({
    queryKey: recordQueryKeys.blueprints(),
    queryFn: ({ signal }) => listRecordBlueprints(signal),
  });

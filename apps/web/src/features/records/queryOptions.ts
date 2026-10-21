import { infiniteQueryOptions, queryOptions } from '@tanstack/react-query';
import {
  getRecordApprovals,
  getRecordForm,
  getRecordPublicationReadiness,
  getRecordPublications,
  getRecordRetentionHolds,
  getResolvedRecordPreview,
  getStatusTransitions,
  type RecordControlCursor,
} from './api';
import { recordQueryKeys } from './queryKeys';

// Revisited virtual rows reuse their result; mutations explicitly invalidate it.
const publicationStaleTimeMs = 60_000;

// Record data loaded ahead, such as a preview whose link is pointed at or a
// side panel's data started with the record, is not fetched again when the
// component showing it mounts; edits invalidate it.
export const readAheadStaleTimeMs = 10_000;

/** A record's values as seen from a context. */
export const resolvedRecordPreviewOptions = (
  recordId: string,
  contextId: string,
) =>
  queryOptions({
    queryKey: recordQueryKeys.resolvedPreview(recordId, contextId),
    queryFn: () => getResolvedRecordPreview(recordId, contextId),
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

/** Continues a newest-first record control list after its last page. */
const nextRecordControlCursor = <T>(
  page: { items: T[]; has_more: boolean },
  cursorOf: (item: T) => RecordControlCursor,
) => {
  const last = page.items.at(-1);
  return page.has_more && last ? cursorOf(last) : undefined;
};

export const recordApprovalsOptions = (recordId: string) =>
  infiniteQueryOptions({
    queryKey: recordQueryKeys.approvals(recordId),
    queryFn: ({ pageParam, signal }) =>
      getRecordApprovals(recordId, pageParam, signal),
    initialPageParam: undefined as RecordControlCursor | undefined,
    getNextPageParam: (page) =>
      nextRecordControlCursor(page, (approval) => ({
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
    initialPageParam: undefined as RecordControlCursor | undefined,
    getNextPageParam: (page) =>
      nextRecordControlCursor(page, (hold) => ({
        before_time: hold.created_at,
        before_id: hold.id,
      })),
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

import { queryOptions } from '@tanstack/react-query';
import {
  getRecordApprovals,
  getRecordForm,
  getRecordPublicationReadiness,
  getRecordPublications,
  getRecordRetentionHolds,
  getStatusTransitions,
} from './api';
import { recordQueryKeys } from './queryKeys';

// Revisited virtual rows reuse their result; mutations explicitly invalidate it.
const publicationStaleTimeMs = 60_000;

/** The record's values and blueprint as an editor needs them. */
export const recordFormOptions = (recordId: string) =>
  queryOptions({
    queryKey: recordQueryKeys.form(recordId),
    queryFn: ({ signal }) => getRecordForm(recordId, signal),
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
  queryOptions({
    queryKey: recordQueryKeys.approvals(recordId),
    queryFn: ({ signal }) => getRecordApprovals(recordId, signal),
  });

export const recordRetentionHoldsOptions = (recordId: string) =>
  queryOptions({
    queryKey: recordQueryKeys.retentionHolds(recordId),
    queryFn: ({ signal }) => getRecordRetentionHolds(recordId, signal),
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

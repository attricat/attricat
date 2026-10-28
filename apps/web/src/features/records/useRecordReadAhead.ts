import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { recordCommentsOptions } from '../record-comments/queryOptions';
import {
  recordFindingCountOptions,
  recordFindingsOptions,
  recordRuleRunsOptions,
} from '../rules/queryOptions';
import {
  blueprintByCodeOptions,
  recordApprovalsOptions,
  recordRetentionHoldsOptions,
} from './queryOptions';
import { sessionOptions } from '../auth/queryOptions';

/**
 * Starts loading what a record's side panels show as soon as the record
 * opens, rather than once its values have rendered, so each panel mounts on
 * data that is already there. Panels the user may not read are skipped, as
 * they are not shown.
 */
export const useRecordReadAhead = (
  recordId: string,
  comments: boolean,
  /** Blueprints of related records whose previews the page shows. */
  relatedBlueprintCodes: readonly string[] = [],
) => {
  const client = useQueryClient();
  const session = useQuery(sessionOptions());
  useEffect(() => {
    void client.prefetchInfiniteQuery(recordApprovalsOptions(recordId));
    void client.prefetchInfiniteQuery(recordRetentionHoldsOptions(recordId));
  }, [client, recordId]);
  useEffect(() => {
    void client.prefetchInfiniteQuery(recordFindingsOptions(recordId));
    void client.prefetchQuery(recordFindingCountOptions(recordId));
    void client.prefetchQuery(recordRuleRunsOptions(recordId));
  }, [client, recordId]);
  // A related preview needs its record's blueprint revision, usually the
  // latest one; loading it by code now lets the preview reuse it instead of
  // loading it after the related record arrives.
  const relatedCodes = relatedBlueprintCodes.join(' ');
  useEffect(() => {
    for (const code of relatedCodes.split(' ').filter(Boolean))
      void client.prefetchQuery(blueprintByCodeOptions(code));
  }, [client, relatedCodes]);
  useEffect(() => {
    if (!comments || !session.data) return;
    void client.prefetchInfiniteQuery(
      recordCommentsOptions(session.data, recordId),
    );
  }, [client, comments, recordId, session.data]);
};

import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { useCan } from '../auth/useCan';
import { recordCommentsOptions } from '../record-comments/queryOptions';
import {
  recordFindingsOptions,
  recordRuleRunsOptions,
} from '../rules/queryOptions';
import {
  recordApprovalsOptions,
  recordRetentionHoldsOptions,
} from './queryOptions';
import { getBlueprintByCode } from './api';
import { recordQueryKeys } from './queryKeys';

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
  const canReadRules = useCan('rules_read');
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  useEffect(() => {
    void client.prefetchQuery(recordApprovalsOptions(recordId));
    void client.prefetchQuery(recordRetentionHoldsOptions(recordId));
  }, [client, recordId]);
  useEffect(() => {
    if (!canReadRules) return;
    void client.prefetchQuery(recordFindingsOptions(recordId));
    void client.prefetchQuery(recordRuleRunsOptions(recordId));
  }, [canReadRules, client, recordId]);
  // A related preview needs its record's blueprint revision, usually the
  // latest one; loading it by code now lets the preview reuse it instead of
  // loading it after the related record arrives.
  const relatedCodes = relatedBlueprintCodes.join(' ');
  useEffect(() => {
    for (const code of relatedCodes.split(' ').filter(Boolean))
      void client.prefetchQuery({
        queryKey: recordQueryKeys.blueprintByCode(code, undefined),
        queryFn: ({ signal }) => getBlueprintByCode(code, undefined, signal),
      });
  }, [client, relatedCodes]);
  useEffect(() => {
    if (!comments || !session.data) return;
    void client.prefetchInfiniteQuery(
      recordCommentsOptions(session.data, recordId),
    );
  }, [client, comments, recordId, session.data]);
};

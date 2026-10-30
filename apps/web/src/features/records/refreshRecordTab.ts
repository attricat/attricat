import type { QueryClient, QueryKey } from '@tanstack/react-query';
import { agentQueryKeys } from '../agents/queryKeys';
import { extensionExportQueryKeys } from '../extension-exports/queryKeys';
import { isRecordCommentsQuery } from '../record-comments/queryKeys';
import { ruleQueryKeys } from '../rules/queryKeys';
import { recordPreviewTabs, type RecordPreviewTabKey } from './constants';
import { recordQueryKeys } from './queryKeys';

const invalidateAll = (client: QueryClient, queryKeys: QueryKey[]) =>
  Promise.all(
    queryKeys.map((queryKey) => client.invalidateQueries({ queryKey })),
  );

/**
 * Refetches what a record tab shows, so opening it shows the latest state
 * rather than what was cached when it was last open.
 */
export const refreshRecordTab = (
  client: QueryClient,
  recordId: string,
  tab: RecordPreviewTabKey,
) => {
  switch (tab) {
    // Unlike after an edit, other records' views of this one are unchanged.
    case recordPreviewTabs.data:
      return invalidateAll(client, [
        recordQueryKeys.form(recordId),
        recordQueryKeys.resolvedPreviews(recordId),
      ]);
    case recordPreviewTabs.dataQuality:
      return invalidateAll(client, [
        ruleQueryKeys.recordRules(recordId),
        ruleQueryKeys.findings(recordId),
        ruleQueryKeys.recordRuns(recordId),
      ]);
    case recordPreviewTabs.comments:
      return client.invalidateQueries({
        predicate: (query) => isRecordCommentsQuery(query.queryKey, recordId),
      });
    case recordPreviewTabs.conversations:
      return invalidateAll(client, [
        agentQueryKeys.recordConversations(recordId),
      ]);
    case recordPreviewTabs.recordControl:
      return invalidateAll(client, [recordQueryKeys.recordControls(recordId)]);
    // Tags and metadata come with the record's form.
    case recordPreviewTabs.systemAnnotations:
      return invalidateAll(client, [recordQueryKeys.form(recordId)]);
    case recordPreviewTabs.exports:
      return invalidateAll(client, [extensionExportQueryKeys.record(recordId)]);
    // Extension tabs read through their own frames; the host caches
    // nothing for them.
    default:
      return Promise.resolve();
  }
};

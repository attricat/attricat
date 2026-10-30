import { QueryClient, type QueryKey } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import { agentQueryKeys } from '../agents/queryKeys';
import { extensionExportQueryKeys } from '../extension-exports/queryKeys';
import { commentQueryKeys } from '../record-comments/queryKeys';
import { ruleQueryKeys } from '../rules/queryKeys';
import { recordPreviewTabs } from './constants';
import { recordQueryKeys } from './queryKeys';
import { refreshRecordTab } from './refreshRecordTab';

const recordId = 'record-1';
const otherRecordId = 'record-2';

/** Built-in tabs, whose content the host caches; extension tabs load their own. */
type CachedTab = (typeof recordPreviewTabs)[keyof typeof recordPreviewTabs];
const cachedTabs = Object.values(recordPreviewTabs);

/** One query each tab shows, for this record and for another. */
const tabQueries = (record: string): Record<CachedTab, QueryKey> => ({
  [recordPreviewTabs.data]: recordQueryKeys.resolvedPreviews(record),
  [recordPreviewTabs.dataQuality]: ruleQueryKeys.findings(record),
  [recordPreviewTabs.comments]: commentQueryKeys.record(
    'workspace',
    'reader',
    record,
  ),
  [recordPreviewTabs.conversations]: agentQueryKeys.recordConversations(record),
  [recordPreviewTabs.recordControl]: recordQueryKeys.approvals(record),
  [recordPreviewTabs.systemAnnotations]: recordQueryKeys.form(record),
  [recordPreviewTabs.exports]: extensionExportQueryKeys.record(record),
});

const cacheWith = (...keys: QueryKey[]) => {
  const client = new QueryClient();
  for (const key of keys) client.setQueryData(key, {});
  return client;
};

const invalidated = (client: QueryClient, key: QueryKey) =>
  client.getQueryCache().find({ queryKey: key, exact: true })?.state
    .isInvalidated;

describe('refreshRecordTab', () => {
  it.each(cachedTabs)(
    'reloads what the %s tab shows for this record only',
    async (tab) => {
      const own = tabQueries(recordId)[tab];
      const other = tabQueries(otherRecordId)[tab];
      const client = cacheWith(own, other);

      await refreshRecordTab(client, recordId, tab);

      expect(invalidated(client, own)).toBe(true);
      expect(invalidated(client, other)).toBe(false);
    },
  );

  it('leaves other tabs cached', async () => {
    const queries = tabQueries(recordId);
    const client = cacheWith(...Object.values(queries));

    await refreshRecordTab(client, recordId, recordPreviewTabs.comments);

    expect(invalidated(client, queries[recordPreviewTabs.comments])).toBe(true);
    expect(invalidated(client, queries[recordPreviewTabs.exports])).toBe(false);
    expect(invalidated(client, queries[recordPreviewTabs.dataQuality])).toBe(
      false,
    );
  });

  it('leaves the cache alone for an extension tab', async () => {
    const queries = tabQueries(recordId);
    const client = cacheWith(...Object.values(queries));

    await refreshRecordTab(client, recordId, 'acme.inventory:stock');

    for (const key of Object.values(queries))
      expect(invalidated(client, key)).toBe(false);
  });
});

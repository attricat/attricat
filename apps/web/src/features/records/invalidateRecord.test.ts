import { QueryClient, QueryObserver } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import {
  invalidateRecord,
  invalidateRecordPublications,
  refreshStaleRecordSearches,
  removeRecord,
} from './invalidateRecord';
import { recordQueryKeys } from './queryKeys';

const projections = [
  recordQueryKeys.form('record'),
  recordQueryKeys.preview('record'),
  recordQueryKeys.resolvedPreview('record', 'context'),
  recordQueryKeys.changes('record'),
  recordQueryKeys.publication('record'),
  recordQueryKeys.publicationReadiness('record'),
  recordQueryKeys.statusTransitions('record', 'context'),
  recordQueryKeys.migrationPreview('record'),
  recordQueryKeys.hierarchy('record', 'context', 'parent'),
  recordQueryKeys.incomingRelationships('record', [], 25),
];
const search = recordQueryKeys.search({
  sort: { field: 'publication_status', direction: 'asc' },
});
const seeded = () => {
  const client = new QueryClient();
  for (const key of [...projections, search]) client.setQueryData(key, []);
  client.setQueryData(recordQueryKeys.form('unrelated'), {});
  return client;
};

describe('record mutation cache policy', () => {
  it('invalidates record projections and search results without invalidating other forms', async () => {
    const client = seeded();
    await invalidateRecord(client, 'record');
    for (const key of [...projections, search])
      expect(client.getQueryState(key)?.isInvalidated).toBe(true);
    expect(
      client.getQueryState(recordQueryKeys.form('unrelated'))?.isInvalidated,
    ).toBe(false);
  });
  it('invalidates publication-dependent search order', async () => {
    const client = seeded();
    await invalidateRecordPublications(client, 'record');
    expect(client.getQueryState(search)?.isInvalidated).toBe(true);
    expect(
      client.getQueryState(recordQueryKeys.publication('record'))
        ?.isInvalidated,
    ).toBe(true);
    expect(
      client.getQueryState(recordQueryKeys.form('record'))?.isInvalidated,
    ).toBe(false);
  });
  it('removes deleted projections instead of fetching a deleted record', async () => {
    const client = seeded();
    await removeRecord(client, 'record');
    for (const key of projections)
      expect(client.getQueryState(key)).toBeUndefined();
    expect(client.getQueryState(search)?.isInvalidated).toBe(true);
  });
  it('keeps searches on screen until they are refreshed after an edit', async () => {
    const client = new QueryClient();
    const fetchSearch = vi.fn().mockResolvedValue([]);
    const observer = new QueryObserver(client, {
      queryKey: search,
      queryFn: fetchSearch,
    });
    const unsubscribe = observer.subscribe(() => undefined);
    await vi.waitFor(() =>
      expect(observer.getCurrentResult().isSuccess).toBe(true),
    );

    await invalidateRecord(client, 'record');
    expect(client.getQueryState(search)?.isInvalidated).toBe(true);
    expect(fetchSearch).toHaveBeenCalledTimes(1);

    await refreshStaleRecordSearches(client);
    expect(fetchSearch).toHaveBeenCalledTimes(2);
    await refreshStaleRecordSearches(client);
    expect(fetchSearch).toHaveBeenCalledTimes(2);
    unsubscribe();
  });
});

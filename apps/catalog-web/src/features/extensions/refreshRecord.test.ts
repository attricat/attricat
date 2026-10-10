import { QueryClient, QueryObserver } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import { recordQueryKeys } from '../records/queryKeys';
import { ruleQueryKeys } from '../rules/queryKeys';
import { refreshCurrentRecord, refreshRecord } from './refreshRecord';

const recordId = '123e4567-e89b-42d3-a456-426614174000';

describe('extension record refresh', () => {
  it('refetches active views of only the context record and invalidates inactive views', async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const active = vi.fn().mockResolvedValue({ values: {} });
    const other = vi.fn().mockResolvedValue({ values: {} });
    const key = recordQueryKeys.resolvedPreview('current', 'context');
    const otherKey = recordQueryKeys.resolvedPreview('other', 'context');
    // Query observers model the mounted record page's active queries.
    const observer = new QueryObserver(client, {
      queryKey: key,
      queryFn: active,
    });
    const stop = observer.subscribe(() => {});
    await observer.refetch();
    client.setQueryData(otherKey, { values: {} });
    const otherObserver = new QueryObserver(client, {
      queryKey: otherKey,
      queryFn: other,
    });
    const stopOther = otherObserver.subscribe(() => {});
    await otherObserver.refetch();
    active.mockClear();
    other.mockClear();
    await refreshRecord(client, 'current');
    expect(active).toHaveBeenCalledTimes(1);
    expect(other).not.toHaveBeenCalled();
    expect(client.getQueryState(key)?.isInvalidated).toBe(false);
    expect(client.getQueryState(otherKey)?.isInvalidated).toBe(false);
    stop();
    stopOther();
    client.clear();
  });

  it('rejects missing grants, other outlets, malformed requests, and missing context', async () => {
    const client = new QueryClient();
    const contribution = {
      capabilities: ['client.refresh'],
      outlet: 'record_action' as const,
    };
    const context = { record_id: recordId };
    const payload = { target: 'current_record' };
    await expect(
      refreshCurrentRecord(
        client,
        { ...contribution, capabilities: [] },
        context,
        payload,
      ),
    ).rejects.toThrow('Refresh denied');
    await expect(
      refreshCurrentRecord(
        client,
        { ...contribution, outlet: null },
        context,
        payload,
      ),
    ).rejects.toThrow('Refresh denied');
    await expect(
      refreshCurrentRecord(client, contribution, context, {
        ...payload,
        record_id: recordId,
      }),
    ).rejects.toThrow();
    await expect(
      refreshCurrentRecord(client, contribution, {}, payload),
    ).rejects.toThrow();
    expect(client.getQueryCache().getAll()).toHaveLength(0);
  });

  it('invalidates record-scoped findings, publications and record controls', async () => {
    const client = new QueryClient();
    const findings = ruleQueryKeys.findings('current');
    const publication = recordQueryKeys.publication('current');
    const approvals = recordQueryKeys.approvals('current');
    client.setQueryData(findings, []);
    client.setQueryData(publication, []);
    client.setQueryData(approvals, []);
    await refreshRecord(client, 'current');
    expect(client.getQueryState(findings)?.isInvalidated).toBe(true);
    expect(client.getQueryState(publication)?.isInvalidated).toBe(true);
    expect(client.getQueryState(approvals)?.isInvalidated).toBe(true);
    client.clear();
  });

  it('refreshes the previewed record of a version 2 selection context', async () => {
    const client = new QueryClient();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    const contribution = {
      capabilities: ['client.refresh'],
      outlet: 'record_action' as const,
    };
    const payload = { target: 'current_record' };
    const selection = {
      context_version: 2,
      selection_source: 'record_preview',
      blueprint_id: recordId,
      blueprint_version: 1,
      context_id: null,
      record_ids: [recordId],
    };
    await refreshCurrentRecord(client, contribution, selection, payload);
    expect(invalidate).toHaveBeenCalledWith(
      { queryKey: recordQueryKeys.form(recordId) },
      { throwOnError: true },
    );
    // Explorer selections and multi-record contexts name no current record.
    await expect(
      refreshCurrentRecord(
        client,
        contribution,
        { ...selection, selection_source: 'explorer_row' },
        payload,
      ),
    ).rejects.toThrow('Refresh denied');
    await expect(
      refreshCurrentRecord(
        client,
        contribution,
        { ...selection, record_ids: [recordId, recordId] },
        payload,
      ),
    ).rejects.toThrow('Refresh denied');
    client.clear();
  });
});

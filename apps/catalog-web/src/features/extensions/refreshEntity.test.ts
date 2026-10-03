import { QueryClient, QueryObserver } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import { entityQueryKeys } from '../entities/queryKeys';
import { ruleQueryKeys } from '../rules/queryKeys';
import { refreshCurrentEntity, refreshEntity } from './refreshEntity';

const entityId = '123e4567-e89b-42d3-a456-426614174000';

describe('extension entity refresh', () => {
  it('refetches active views of only the context entity and invalidates inactive views', async () => {
    const client = new QueryClient({
      defaultOptions: { queries: { retry: false } },
    });
    const active = vi.fn().mockResolvedValue({ values: {} });
    const other = vi.fn().mockResolvedValue({ values: {} });
    const key = entityQueryKeys.resolvedPreview('current', 'context');
    const otherKey = entityQueryKeys.resolvedPreview('other', 'context');
    // Query observers model the mounted entity page's active queries.
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
    await refreshEntity(client, 'current');
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
      outlet: 'entity_action' as const,
    };
    const context = { entity_id: entityId };
    const payload = { target: 'current_entity' };
    await expect(
      refreshCurrentEntity(
        client,
        { ...contribution, capabilities: [] },
        context,
        payload,
      ),
    ).rejects.toThrow('Refresh denied');
    await expect(
      refreshCurrentEntity(
        client,
        { ...contribution, outlet: null },
        context,
        payload,
      ),
    ).rejects.toThrow('Refresh denied');
    await expect(
      refreshCurrentEntity(client, contribution, context, {
        ...payload,
        entity_id: entityId,
      }),
    ).rejects.toThrow();
    await expect(
      refreshCurrentEntity(client, contribution, {}, payload),
    ).rejects.toThrow();
    expect(client.getQueryCache().getAll()).toHaveLength(0);
  });

  it('invalidates entity-scoped findings and publications', async () => {
    const client = new QueryClient();
    const findings = ruleQueryKeys.findings('current');
    const publication = entityQueryKeys.publication('current');
    client.setQueryData(findings, []);
    client.setQueryData(publication, []);
    await refreshEntity(client, 'current');
    expect(client.getQueryState(findings)?.isInvalidated).toBe(true);
    expect(client.getQueryState(publication)?.isInvalidated).toBe(true);
    client.clear();
  });

  it('refreshes the previewed entity of a version 2 selection context', async () => {
    const client = new QueryClient();
    const invalidate = vi.spyOn(client, 'invalidateQueries');
    const contribution = {
      capabilities: ['client.refresh'],
      outlet: 'entity_action' as const,
    };
    const payload = { target: 'current_entity' };
    const selection = {
      context_version: 2,
      selection_source: 'entity_preview',
      blueprint_id: entityId,
      blueprint_version: 1,
      context_id: null,
      entity_ids: [entityId],
    };
    await refreshCurrentEntity(client, contribution, selection, payload);
    expect(invalidate).toHaveBeenCalledWith(
      { queryKey: entityQueryKeys.form(entityId) },
      { throwOnError: true },
    );
    // Explorer selections and multi-entity contexts name no current entity.
    await expect(
      refreshCurrentEntity(
        client,
        contribution,
        { ...selection, selection_source: 'explorer_row' },
        payload,
      ),
    ).rejects.toThrow('Refresh denied');
    await expect(
      refreshCurrentEntity(
        client,
        contribution,
        { ...selection, entity_ids: [entityId, entityId] },
        payload,
      ),
    ).rejects.toThrow('Refresh denied');
    client.clear();
  });
});

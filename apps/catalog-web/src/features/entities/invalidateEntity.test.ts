import { QueryClient } from '@tanstack/react-query';
import { describe, expect, it } from 'vitest';
import {
  invalidateEntity,
  invalidateEntityPublications,
  removeEntity,
} from './invalidateEntity';
import { entityQueryKeys } from './queryKeys';

const projections = [
  entityQueryKeys.form('entity'),
  entityQueryKeys.preview('entity'),
  entityQueryKeys.resolvedPreview('entity', 'context'),
  entityQueryKeys.changes('entity'),
  entityQueryKeys.publication('entity'),
  entityQueryKeys.migrationPreview('entity'),
  entityQueryKeys.hierarchy('entity', 'context', 'parent'),
  entityQueryKeys.incomingRelationships('entity', [], 25),
];
const search = entityQueryKeys.search({
  sort: { field: 'publication_status', direction: 'asc' },
});
const seeded = () => {
  const client = new QueryClient();
  for (const key of [...projections, search]) client.setQueryData(key, []);
  client.setQueryData(entityQueryKeys.form('unrelated'), {});
  return client;
};

describe('entity mutation cache policy', () => {
  it('invalidates entity projections and search results without invalidating other forms', async () => {
    const client = seeded();
    await invalidateEntity(client, 'entity');
    for (const key of [...projections, search])
      expect(client.getQueryState(key)?.isInvalidated).toBe(true);
    expect(
      client.getQueryState(entityQueryKeys.form('unrelated'))?.isInvalidated,
    ).toBe(false);
  });
  it('invalidates publication-dependent search order', async () => {
    const client = seeded();
    await invalidateEntityPublications(client, 'entity');
    expect(client.getQueryState(search)?.isInvalidated).toBe(true);
    expect(
      client.getQueryState(entityQueryKeys.publication('entity'))
        ?.isInvalidated,
    ).toBe(true);
    expect(
      client.getQueryState(entityQueryKeys.form('entity'))?.isInvalidated,
    ).toBe(false);
  });
  it('removes deleted projections instead of fetching a deleted entity', async () => {
    const client = seeded();
    await removeEntity(client, 'entity');
    for (const key of projections)
      expect(client.getQueryState(key)).toBeUndefined();
    expect(client.getQueryState(search)?.isInvalidated).toBe(true);
  });
});

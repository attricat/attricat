import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  getBlueprintRevision,
  listBlueprintRevisions,
  publishBlueprintRevision,
  listBlueprints,
} from './api';

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const blueprint = {
  id: blueprintId,
  code: 'product',
  name: 'Product',
  kind: 'entity',
  version: 2,
  includes: [],
  display: {},
  views: {},
  entity_schema: null,
  status: 'published',
  published_at: '2026-08-19T12:00:00Z',
  created_at: '2026-08-19T11:00:00Z',
  updated_at: '2026-08-19T12:00:00Z',
  deleted_at: null,
  definition: 'format_version = 1',
  definition_hash: 'abc123',
};
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => fetchMock.mockReset());

const respond = (body: unknown) => {
  fetchMock.mockResolvedValue({ ok: true, json: () => Promise.resolve(body) });
};

describe('blueprint API client', () => {
  it('loads the catalogue, history, and a selected revision', async () => {
    respond([blueprint]);
    await listBlueprints();
    expect(fetchMock).toHaveBeenLastCalledWith('/api/blueprints/catalogue');

    respond([blueprint]);
    await listBlueprintRevisions(blueprintId);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions`,
    );

    respond({ blueprint, attributes: [] });
    await getBlueprintRevision(blueprintId, 2);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions/2`,
    );

    respond({ blueprint, attributes: [] });
    await publishBlueprintRevision(blueprintId, 2);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions/2/publish`,
      { method: 'POST' },
    );
  });
});

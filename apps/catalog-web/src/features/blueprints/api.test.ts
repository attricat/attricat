import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiRequestError } from '../../api/request';
import {
  createBlueprint,
  createBlueprintRevision,
  getBlueprintRevision,
  listBlueprintMigrationBatches,
  listBlueprintRevisions,
  publishBlueprintRecords,
  publishBlueprintRecordsAllChannels,
  publishBlueprintRevision,
  startSafeBlueprintMigrationBatch,
  listBlueprints,
} from './api';

const blueprintId = '123e4567-e89b-12d3-a456-426614174000';
const blueprint = {
  id: blueprintId,
  code: 'product',
  name: 'Product',
  kind: 'record',
  version: 2,
  includes: [],
  display: {},
  views: {},
  record_schema: null,
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

    const controller = new AbortController();
    respond([blueprint]);
    await listBlueprintRevisions(blueprintId, controller.signal);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions`,
      { signal: controller.signal },
    );

    respond({ blueprint, attributes: [] });
    await getBlueprintRevision(blueprintId, 2);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions/2`,
    );

    respond({ blueprint, attributes: [] });
    await createBlueprint('format_version = 1');
    expect(fetchMock).toHaveBeenLastCalledWith('/api/blueprints', {
      body: JSON.stringify({ definition: 'format_version = 1' }),
      headers: { 'Content-Type': 'application/json' },
      method: 'POST',
    });

    respond({ blueprint, attributes: [] });
    await createBlueprintRevision(blueprintId, 'format_version = 1');
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions`,
      {
        body: JSON.stringify({ definition: 'format_version = 1' }),
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
      },
    );

    const publicationSummary = {
      record_count: 2,
      channel_count: 1,
      publication_count: 2,
    };
    respond(publicationSummary);
    await publishBlueprintRecords(blueprintId, 2, blueprintId);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions/2/record-publications`,
      {
        body: JSON.stringify({ context_id: blueprintId }),
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
      },
    );

    respond(publicationSummary);
    await publishBlueprintRecordsAllChannels(blueprintId, 2);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions/2/record-publications/publish-all`,
      { method: 'POST' },
    );

    const migrationBatch = {
      id: blueprintId,
      blueprint_id: blueprintId,
      target_version: 2,
      status: 'queued',
      created_at: '2026-10-05T12:00:00Z',
      started_at: null,
      completed_at: null,
    };
    respond([
      {
        ...migrationBatch,
        total_records: 10_000,
        processed_records: 42,
        migrated_records: 40,
        needs_input_records: 1,
        failed_records: 1,
      },
    ]);
    await expect(
      listBlueprintMigrationBatches(blueprintId),
    ).resolves.toMatchObject([
      { status: 'queued', total_records: 10_000, processed_records: 42 },
    ]);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/migration-batches`,
    );

    respond(migrationBatch);
    await expect(
      startSafeBlueprintMigrationBatch(blueprintId, 2),
    ).resolves.toMatchObject({ status: 'queued' });
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions/2/safe-migration-batches`,
      { method: 'POST' },
    );

    respond({ blueprint, attributes: [] });
    await publishBlueprintRevision(blueprintId, 2);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/blueprints/${blueprintId}/versions/2/publish`,
      { method: 'POST' },
    );
  });

  it('normalizes null legacy table fields in rich-column save responses', async () => {
    const richTableResponse = {
      blueprint: {
        ...blueprint,
        views: {
          table: {
            type: 'table',
            fields: null,
            columns: [{ field: 'name', label: 'Product name' }],
          },
        },
      },
      attributes: [],
    };

    respond(richTableResponse);
    await expect(createBlueprint('format_version = 1')).resolves.toMatchObject({
      blueprint: {
        views: {
          table: { fields: [], columns: [{ field: 'name' }] },
        },
      },
    });

    respond(richTableResponse);
    await expect(
      createBlueprintRevision(blueprintId, 'format_version = 1'),
    ).resolves.toMatchObject({
      blueprint: {
        views: {
          table: { fields: [], columns: [{ field: 'name' }] },
        },
      },
    });
  });

  it('preserves structured validation errors for editor feedback', async () => {
    fetchMock.mockResolvedValue({
      json: () =>
        Promise.resolve({
          error: {
            code: 'invalid_blueprint_definition',
            message: 'blueprints must define at least one attribute',
          },
        }),
      ok: false,
      status: 422,
    });

    await expect(createBlueprint('not valid TOML')).rejects.toMatchObject({
      name: 'ApiRequestError',
      status: 422,
      code: 'invalid_blueprint_definition',
      message: 'blueprints must define at least one attribute',
    } satisfies Partial<ApiRequestError>);
  });
});

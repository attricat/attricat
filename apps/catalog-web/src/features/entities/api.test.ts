import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createEntity,
  deleteEntity,
  getBlueprintByCode,
  getBlueprintRevision,
  getEntityChanges,
  getEntityForm,
  getEntityHierarchy,
  getIncomingRelationships,
  getEntityPublications,
  listEntityBlueprints,
  publishEntity,
  publishEntityAllChannels,
  previewEntityMigration,
  getResolvedEntityPreview,
  searchEntities,
  smartFillEntityForm,
  unpublishEntity,
  updateEntity,
} from './api';

const entityId = '123e4567-e89b-12d3-a456-426614174000';
const blueprintId = '223e4567-e89b-12d3-a456-426614174000';
const blueprint = {
  code: 'product',
  id: blueprintId,
  name: 'Product',
  status: 'published',
  version: 2,
  display: {},
};
const blueprintWithAttributes = { blueprint, attributes: [] };

const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);

afterEach(() => {
  fetchMock.mockReset();
});

const respond = (body: unknown) => {
  fetchMock.mockResolvedValue({ ok: true, json: () => Promise.resolve(body) });
};

const respondError = (status: number, body: unknown) => {
  fetchMock.mockResolvedValue({
    ok: false,
    status,
    json: () => Promise.resolve(body),
  });
};

describe('entity API client', () => {
  it('sends active-context source text to Smart Fill', async () => {
    respond({ fields: { title: 'Trail shoes' } });

    await expect(
      smartFillEntityForm({
        entity_id: entityId,
        context_id: null,
        is_default_context: true,
        content: 'Trail shoes',
      }),
    ).resolves.toEqual({ fields: { title: 'Trail shoes' } });

    expect(fetchMock).toHaveBeenCalledWith('/api/agent/smart-fill', {
      body: JSON.stringify({
        entity_id: entityId,
        context_id: null,
        is_default_context: true,
        content: 'Trail shoes',
      }),
      headers: { 'Content-Type': 'application/json' },
      method: 'POST',
    });
  });

  it('loads blueprints and entities from their code-based routes', async () => {
    respond(blueprintWithAttributes);
    await getBlueprintByCode('summer sale', 2);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/blueprints/by-code/summer%20sale/versions/2',
    );

    respond({
      entity: {
        blueprint_id: blueprintId,
        blueprint_version: 2,
        id: entityId,
        is_sample: false,
      },
      blueprint: blueprintWithAttributes,
      values: [],
      context: { default: {} },
    });
    await getEntityForm(entityId);
    expect(fetchMock).toHaveBeenCalledWith(`/api/v1/entities/${entityId}`);
  });

  it('soft-deletes via the entity DELETE endpoint without parsing a response body', async () => {
    fetchMock.mockResolvedValue({ ok: true, status: 204 });
    await expect(deleteEntity(entityId)).resolves.toBeUndefined();
    expect(fetchMock).toHaveBeenCalledWith(`/api/entities/${entityId}`, {
      method: 'DELETE',
    });
  });

  it('uses the publication endpoint contracts', async () => {
    const publication = {
      context_id: entityId,
      context_code: 'web',
      status: 'not_published',
      published_at: null,
      published_by_user_id: null,
    };
    respond([publication]);
    await getEntityPublications(entityId);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/v1/entities/${entityId}/publications`,
    );

    respond({
      ...publication,
      status: 'published',
      published_at: '2026-09-30T12:00:00.000Z',
      published_by_user_id: entityId,
    });
    await publishEntity(entityId, entityId);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/v1/entities/${entityId}/publications`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ context_id: entityId }),
      },
    );

    respond([
      {
        ...publication,
        status: 'published',
        published_at: '2026-09-30T12:00:00.000Z',
        published_by_user_id: entityId,
      },
    ]);
    await publishEntityAllChannels(entityId);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/v1/entities/${entityId}/publications/publish-all`,
      { method: 'POST' },
    );

    respond(undefined);
    await unpublishEntity(entityId, entityId);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/v1/entities/${entityId}/publications/unpublish`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ context_id: entityId }),
      },
    );
  });

  it('accepts extension component IDs containing hyphens', async () => {
    respond({
      blueprint: {
        ...blueprint,
        views: {
          table: {
            type: 'table',
            columns: [
              {
                field: 'price',
                renderer: {
                  id: 'attricat-extension-example.table-cell',
                  version: 1,
                },
              },
            ],
          },
        },
      },
      attributes: [],
    });

    await expect(getBlueprintRevision(entityId, 2)).resolves.toBeDefined();
  });

  it('loads entity changes from the timeline route', async () => {
    respond([
      {
        audit_event_id: entityId,
        occurred_at: '2026-09-09T12:00:00Z',
        actor_user_id: null,
        actor_display_name: null,
        actor_email: null,
        executor_type: 'human',
        agent_run_id: null,
        approval_decision: null,
        approved_by_user_id: null,
        approved_by_display_name: null,
        attribute_id: entityId,
        attribute_code: 'title',
        context_id: null,
        context_code: null,
        change_kind: 'replace',
        before_value: 'Old title',
        after_value: 'New title',
      },
    ]);
    await expect(getEntityChanges(entityId)).resolves.toHaveLength(1);
    expect(fetchMock).toHaveBeenCalledWith(`/api/entities/${entityId}/changes`);
  });

  it('accepts file values in a migration preview', async () => {
    respond({
      migration_id: entityId,
      source_version: 1,
      target: blueprintWithAttributes,
      values: [
        {
          kind: 'file',
          attribute_code: 'image',
          context_id: null,
          files: [
            {
              id: entityId,
              filename: 'product.png',
              mime_type: 'image/png',
              byte_size: 42,
              sha256: 'a'.repeat(64),
              status: 'ready',
              variants: [],
            },
          ],
        },
      ],
      status: 'ready',
      issues: [],
    });

    await expect(previewEntityMigration(entityId)).resolves.toMatchObject({
      values: [{ kind: 'file', attribute_code: 'image' }],
    });
  });

  it('posts and puts the entity payload contract', async () => {
    const entity = {
      blueprint_id: blueprintId,
      blueprint_version: 2,
      id: entityId,
      is_sample: false,
    };
    respond(entity);
    await createEntity({
      blueprint: { code: 'product', version: 2 },
      values: [],
    });
    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product', version: 2 },
        values: [],
      }),
    });

    respond(entity);
    await updateEntity(entityId, { values: [], relationships: [] });
    expect(fetchMock).toHaveBeenLastCalledWith(`/api/v1/entities/${entityId}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        values: [],
        relationships: [],
        remove_values: [],
      }),
    });
  });

  it('forwards cancellation signals for explorer searches and facets', async () => {
    const controller = new AbortController();

    respond(blueprintWithAttributes);
    await getBlueprintByCode('product', undefined, controller.signal);
    expect(fetchMock).toHaveBeenLastCalledWith(
      '/api/blueprints/by-code/product',
      { signal: controller.signal },
    );

    respond([blueprint]);
    await listEntityBlueprints(controller.signal);
    expect(fetchMock).toHaveBeenLastCalledWith('/api/blueprints', {
      signal: controller.signal,
    });

    respond({
      blueprint: blueprintWithAttributes,
      items: [],
      next_cursor: null,
    });
    await searchEntities({
      blueprint: 'product',
      signal: controller.signal,
    });
    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [],
        page: { size: 25, cursor: null },
      }),
      signal: controller.signal,
    });
  });

  it('uses the backend default ordering', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [
        {
          id: entityId,
          blueprint_version: 1,
          schema_outdated: true,
          is_sample: false,
          display: { default: 'Legacy product' },
          preview: { default: { title: 'Legacy product' } },
        },
      ],
      next_cursor: null,
    });
    const result = await searchEntities({ blueprint: 'product' });
    expect(result.items[0]).toMatchObject({
      blueprint_version: 1,
      schema_outdated: true,
    });
    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [],
        page: { size: 25, cursor: null },
      }),
    });
  });

  it('forwards structured query text and parses match explanations', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [
        {
          id: entityId,
          blueprint_version: 1,
          schema_outdated: false,
          is_sample: false,
          display: { default: 'Red product' },
          preview: { default: { title: 'Red product' } },
          match_explanations: [
            {
              term: 'color.name:red',
              matching_entity_id: entityId,
              matching_attribute_code: 'name',
              traversal_depth: 1,
              relationship_path: [
                {
                  source_entity_id: entityId,
                  attribute_code: 'color',
                  target_entity_id: entityId,
                },
              ],
            },
          ],
        },
      ],
      next_cursor: null,
    });
    const result = await searchEntities({
      blueprint: 'product',
      query: 'color.name:red',
    });
    expect(result.items[0].match_explanations[0]?.term).toBe('color.name:red');
    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: 'color.name:red',
        filters: [],
        page: { size: 25, cursor: null },
      }),
    });
  });

  it('requests a total only for the first Explorer page', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [],
      next_cursor: null,
      total_count: 500,
      total_count_capped: true,
    });

    const result = await searchEntities({
      blueprint: 'product',
      includeTotal: true,
    });

    expect(result).toMatchObject({
      total_count: 500,
      total_count_capped: true,
    });
    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [],
        include_total: true,
        page: { size: 25, cursor: null },
      }),
    });
  });

  it('posts an opaque search cursor for subsequent pages', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [],
      next_cursor: null,
    });

    await searchEntities({
      blueprint: 'product',
      cursor: 'opaque-next-cursor',
    });

    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [],
        page: { size: 25, cursor: 'opaque-next-cursor' },
      }),
    });
  });

  it('posts configured sort state for Explorer table columns', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [],
      next_cursor: null,
    });

    await searchEntities({
      blueprint: 'product',
      sort: { field: 'category.name', direction: 'desc' },
    });

    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [],
        sort: { field: 'category.name', direction: 'desc' },
        page: { size: 25, cursor: null },
      }),
    });
  });

  it('posts typed attribute filters', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [],
      next_cursor: null,
    });

    await searchEntities({
      blueprint: 'product',
      filters: [{ field: 'price', operator: 'gte', value: 100 }],
    });

    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [{ field: 'price', operator: 'gte', value: 100 }],
        page: { size: 25, cursor: null },
      }),
    });
  });

  it('posts relationship-path filters', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [],
      next_cursor: null,
    });

    await searchEntities({
      blueprint: 'product',
      relationshipFilters: [
        { field: 'brand.owner', selected_target_ids: [entityId] },
      ],
    });

    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [],
        relationship_filters: [
          { field: 'brand.owner', selected_target_ids: [entityId] },
        ],
        page: { size: 25, cursor: null },
      }),
    });
  });

  it('loads incoming relationship pages with the configured selectors', async () => {
    respond({
      items: [
        {
          id: entityId,
          blueprint_code: 'product',
          blueprint_version: 1,
          is_sample: false,
          display: { default: 'Navy shirt' },
        },
      ],
      next_cursor: 'next-page',
    });

    await getIncomingRelationships(
      entityId,
      [{ source_blueprint: 'product', field: 'categories' }],
      10,
      null,
    );

    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/v1/entities/${entityId}/incoming-relationships`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          relationships: [{ source_blueprint: 'product', field: 'categories' }],
          page: { size: 10, cursor: null },
        }),
      },
    );
  });

  it('loads resolved previews with the entity revision identity', async () => {
    respond({
      entity: {
        id: entityId,
        blueprint_id: entityId,
        blueprint_version: 1,
        is_sample: false,
      },
      requested_context: {
        id: entityId,
        code: 'default',
        data: {},
        parent_id: null,
      },
      values: {},
    });

    await getResolvedEntityPreview(entityId, entityId);

    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/entities/${entityId}/resolved-preview?context_id=${entityId}`,
    );
  });

  it('loads a hierarchy for one self-referential relationship', async () => {
    respond({
      items: [{ id: entityId, display: 'Catalog' }],
      paths: [[{ id: entityId, display: 'Catalog' }]],
      truncated: false,
      multiple_parents: false,
      cycle_detected: false,
    });

    await getEntityHierarchy(entityId, entityId, 'parent category');

    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/entities/${entityId}/hierarchy?context_id=${entityId}&field=parent%20category`,
    );
  });

  it('rejects malformed successful responses', async () => {
    respond({ id: 'not-a-uuid' });

    await expect(
      createEntity({ blueprint: { code: 'product' }, values: [] }),
    ).rejects.toThrow('Invalid API response');
  });

  it('preserves structured server validation errors', async () => {
    respondError(422, {
      error: {
        code: 'entity_schema_mismatch',
        message: 'title is required',
      },
    });

    await expect(
      createEntity({ blueprint: { code: 'product' }, values: [] }),
    ).rejects.toMatchObject({
      name: 'ApiRequestError',
      status: 422,
      code: 'entity_schema_mismatch',
      message: 'title is required',
    });
  });

  it('falls back to a status message for malformed error responses', async () => {
    respondError(500, { unexpected: true });

    await expect(
      createEntity({ blueprint: { code: 'product' }, values: [] }),
    ).rejects.toThrow('Request failed (500)');
  });

  it('rejects invalid request inputs before fetching', async () => {
    expect(() => getEntityForm('not-a-uuid')).toThrow('Invalid UUID');
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

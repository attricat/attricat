import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createEntity,
  createContext,
  getBlueprintByCode,
  getEntityForm,
  getEntityHierarchy,
  getIncomingRelationships,
  getRelationshipTreeFacetChildren,
  getResolvedEntityPreview,
  searchEntities,
  updateEntity,
} from './api';

const entityId = '123e4567-e89b-12d3-a456-426614174000';
const blueprint = {
  code: 'product',
  name: 'Product',
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
  it('loads blueprints and entities from their code-based routes', async () => {
    respond(blueprintWithAttributes);
    await getBlueprintByCode('summer sale', 2);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/blueprints/by-code/summer%20sale/versions/2',
      undefined,
    );

    respond({
      entity: { id: entityId },
      blueprint: blueprintWithAttributes,
      values: [],
      context: { default: {} },
    });
    await getEntityForm(entityId);
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/v1/entities/${entityId}`,
      undefined,
    );
  });

  it('posts and puts the entity payload contract', async () => {
    respond({ id: entityId });
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

    respond({ id: entityId });
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

  it('uses the backend default ordering', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [
        {
          id: entityId,
          blueprint_version: 1,
          schema_outdated: true,
          display: { default: 'Legacy product' },
          preview: { default: { title: 'Legacy product' } },
        },
      ],
      next_cursor: null,
    });
    const result = await searchEntities('product', undefined, '', null);
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

  it('posts an opaque search cursor for subsequent pages', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [],
      next_cursor: null,
    });

    await searchEntities('product', undefined, '', 'opaque-next-cursor');

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

  it('posts relationship-tree facet selections', async () => {
    respond({
      blueprint: blueprintWithAttributes,
      items: [],
      next_cursor: null,
    });

    await searchEntities('product', undefined, '', null, {
      source_relationship_field: 'categories',
      hierarchy_field: 'parent',
      context_id: entityId,
      selected_target_ids: [entityId],
    });

    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [],
        relationship_tree_facet: {
          source_relationship_field: 'categories',
          hierarchy_field: 'parent',
          context_id: entityId,
          selected_target_ids: [entityId],
        },
        page: { size: 25, cursor: null },
      }),
    });
  });

  it('loads a paginated relationship-tree child page', async () => {
    respond({
      items: [
        { id: entityId, display: 'Clothing', count: 4, has_children: true },
      ],
      next_cursor: null,
    });

    await getRelationshipTreeFacetChildren({
      blueprint: { code: 'product' },
      source_relationship_field: 'categories',
      hierarchy_field: 'parent',
      context_id: entityId,
      parent_id: entityId,
      cursor: null,
    });

    expect(fetchMock).toHaveBeenLastCalledWith(
      '/api/v1/entities/facets/relationship-tree/children',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          blueprint: { code: 'product' },
          source_relationship_field: 'categories',
          hierarchy_field: 'parent',
          context_id: entityId,
          parent_id: entityId,
          cursor: null,
        }),
      },
    );
  });

  it('loads incoming relationship pages with the configured selectors', async () => {
    respond({
      items: [
        {
          id: entityId,
          blueprint_code: 'product',
          blueprint_version: 1,
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
      undefined,
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
      undefined,
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
    expect(() => createContext('en GB', {}, entityId)).toThrow('hyphens');
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

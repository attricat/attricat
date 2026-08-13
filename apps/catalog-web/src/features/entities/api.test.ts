import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  createEntity,
  createContext,
  getBlueprintByCode,
  getEntityForm,
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
    const result = await searchEntities('product', undefined, '');
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

  it('rejects malformed successful responses', async () => {
    respond({ id: 'not-a-uuid' });

    await expect(
      createEntity({ blueprint: { code: 'product' }, values: [] }),
    ).rejects.toThrow('Invalid API response');
  });

  it('rejects invalid request inputs before fetching', async () => {
    expect(() => getEntityForm('not-a-uuid')).toThrow('Invalid UUID');
    expect(() => createContext('en-GB', {})).toThrow('underscores');
    expect(fetchMock).not.toHaveBeenCalled();
  });
});

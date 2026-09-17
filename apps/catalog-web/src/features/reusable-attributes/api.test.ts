import { afterEach, describe, expect, it, vi } from 'vitest';
import {
  attachReusableAttribute,
  attachReusableAttributeGroup,
  createReusableAttribute,
  createReusableAttributeGroup,
  createReusableAttributeRevision,
  listReusableAttributes,
  publishReusableAttributeRevision,
} from './api';

const id = '123e4567-e89b-12d3-a456-426614174000';
const attribute = {
  id,
  definition_id: id,
  namespace: 'catalog',
  code: 'material',
  name: 'Material',
  version: 1,
  value_type: 'string',
  value_schema: null,
  default_value: null,
  file_policy: null,
  target_blueprint_code: null,
  cardinality: null,
  target_cardinality: null,
  tags: [],
  context_fallback: 'default',
  context_editable: 'all',
  readonly: false,
  searchable: false,
  facetable: false,
  status: 'draft',
  published_at: null,
};
const input = {
  namespace: 'catalog',
  code: 'material',
  name: 'Material',
  value_type: 'string' as const,
  tags: [],
  context_fallback: 'default' as const,
  context_editable: 'all' as const,
  readonly: false,
  searchable: false,
  facetable: false,
};
const fetchMock = vi.fn();
vi.stubGlobal('fetch', fetchMock);
afterEach(() => fetchMock.mockReset());
const respond = (body: unknown) =>
  fetchMock.mockResolvedValue({ ok: true, json: () => Promise.resolve(body) });

describe('reusable attribute API client', () => {
  it('uses the revision and publication endpoint contracts', async () => {
    respond([attribute]);
    await listReusableAttributes(true);
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/reusable-attributes?include_drafts=true',
    );

    respond(attribute);
    await createReusableAttribute(input);
    expect(fetchMock).toHaveBeenLastCalledWith('/api/reusable-attributes', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(input),
    });

    respond(attribute);
    await createReusableAttributeRevision(id, input);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/reusable-attributes/${id}/versions`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(input),
      },
    );

    respond({
      ...attribute,
      status: 'published',
      published_at: '2026-01-01T00:00:00.000Z',
    });
    await publishReusableAttributeRevision(id);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/reusable-attribute-revisions/${id}/publish`,
      { method: 'POST' },
    );
  });

  it('creates groups and attaches published revisions to entities', async () => {
    respond({
      id,
      code: 'dimensions',
      name: 'Dimensions',
      position: 0,
      reusable_attribute_revision_ids: [id],
    });
    await createReusableAttributeGroup({
      code: 'dimensions',
      name: 'Dimensions',
      position: 0,
      reusable_attribute_revision_ids: [id],
    });
    expect(fetchMock).toHaveBeenLastCalledWith(
      '/api/reusable-attribute-groups',
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          code: 'dimensions',
          name: 'Dimensions',
          position: 0,
          reusable_attribute_revision_ids: [id],
        }),
      },
    );

    respond({});
    await attachReusableAttribute(id, id);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/v1/entities/${id}/reusable-attributes`,
      {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ reusable_attribute_revision_id: id }),
      },
    );

    respond([]);
    await attachReusableAttributeGroup(id, id);
    expect(fetchMock).toHaveBeenLastCalledWith(
      `/api/v1/entities/${id}/reusable-attribute-groups/${id}`,
      { method: 'POST' },
    );
  });
});

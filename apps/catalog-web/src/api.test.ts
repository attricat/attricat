import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  createEntity,
  getBlueprintByCode,
  getEntityForm,
  searchEntities,
  updateEntity,
} from './api'

const fetchMock = vi.fn()
vi.stubGlobal('fetch', fetchMock)

afterEach(() => {
  fetchMock.mockReset()
})

function respond(body: unknown) {
  fetchMock.mockResolvedValue({ ok: true, json: () => Promise.resolve(body) })
}

describe('entity API client', () => {
  it('loads blueprints and entity forms from their code-based routes', async () => {
    respond({})
    await getBlueprintByCode('summer sale', 2)
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/blueprints/by-code/summer%20sale/versions/2',
      undefined,
    )

    respond({})
    await getEntityForm('entity/id')
    expect(fetchMock).toHaveBeenCalledWith(
      '/api/v1/entities/entity%2Fid/form',
      undefined,
    )
  })

  it('posts and puts the entity payload contract', async () => {
    respond({ id: 'created' })
    await createEntity({
      blueprint: { code: 'product', version: 2 },
      values: [],
    })
    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product', version: 2 },
        values: [],
      }),
    })

    respond({ id: 'updated' })
    await updateEntity('updated', { values: [], relationships: [] })
    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/updated', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ values: [], relationships: [] }),
    })
  })

  it('uses the backend default sort', async () => {
    respond({})
    await searchEntities('product', undefined, '')
    expect(fetchMock).toHaveBeenLastCalledWith('/api/v1/entities/search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        blueprint: { code: 'product' },
        query: '',
        filters: [],
        sort: [],
        page: { size: 25, cursor: null },
      }),
    })
  })
})

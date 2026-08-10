export type Attribute = { code: string; value_type: string; [key: string]: unknown }
export type Blueprint = { code: string; version: number; [key: string]: unknown }
export type EntityItem = { id: string; code: string; preview: Record<string, unknown> }
export type EntitySearchResponse = {
  blueprint: { blueprint: Blueprint; attributes: Attribute[] }
  items: EntityItem[]
  next_cursor: string | null
}

export type EntityPreview = { id?: string; code?: string; preview?: Record<string, unknown>; [key: string]: unknown }

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, init)
  if (!response.ok) throw new Error(`Request failed (${response.status})`)
  return response.json() as Promise<T>
}

export function searchEntities(blueprint: string, version: number | undefined, query: string) {
  return request<EntitySearchResponse>('/api/v1/entities/search', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      blueprint: { code: blueprint, ...(version === undefined ? {} : { version }) },
      query,
      filters: [],
      sort: [{ field: 'code', direction: 'asc' }],
      page: { size: 25, cursor: null },
    }),
  })
}

export function getEntityPreview(id: string) {
  return request<EntityPreview>(`/api/entities/${encodeURIComponent(id)}/projections/preview`)
}

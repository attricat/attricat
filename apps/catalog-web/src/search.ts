import type { Attribute, EntityPreview } from './api'

export type ExplorerSearch = {
  blueprint?: string
  version?: number
  query?: string
}

export function parseExplorerSearch(
  input: Record<string, unknown>,
): ExplorerSearch {
  const blueprint =
    typeof input.blueprint === 'string' && input.blueprint.trim()
      ? input.blueprint
      : undefined
  const query =
    typeof input.query === 'string' && input.query.trim()
      ? input.query
      : undefined
  const versionValue =
    typeof input.version === 'string' ? Number(input.version) : input.version
  const version =
    typeof versionValue === 'number' &&
    Number.isInteger(versionValue) &&
    versionValue > 0
      ? versionValue
      : undefined
  return { blueprint, version, query }
}

export function displayValue(value: unknown): string {
  if (value === null || value === undefined) return '—'
  if (typeof value === 'object') return JSON.stringify(value)
  return String(value)
}

export function previewHeading(
  preview: EntityPreview | undefined,
  attributes: Attribute[],
  entityId: string,
): string {
  const displayAttribute = attributes.find((attribute) =>
    attribute.tags.includes('display'),
  )
  const value = displayAttribute && preview?.default?.[displayAttribute.code]
  return value === undefined || value === null || value === ''
    ? entityId
    : displayValue(value)
}

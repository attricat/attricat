import type { Attribute, NewAttributeValue, RelationshipTargets } from './api'

export function valuesForForm(
  attributes: Attribute[],
  values: NewAttributeValue[],
): Record<string, string> {
  return Object.fromEntries(
    attributes.map((attribute) => {
      const matching = values.filter(
        (value) => value.attribute_code === attribute.code,
      )
      if (attribute.value_type === 'relationship') {
        return [
          attribute.code,
          matching
            .filter(
              (
                value,
              ): value is Extract<
                NewAttributeValue,
                { kind: 'relationship' }
              > => value.kind === 'relationship',
            )
            .map((value) => value.target_entity_id)
            .join(', '),
        ]
      }
      const scalar = matching.find(
        (value): value is Extract<NewAttributeValue, { kind: 'scalar' }> =>
          value.kind === 'scalar',
      )
      return [attribute.code, scalar?.value ?? '']
    }),
  )
}

export function serializeAttributeValues(
  attributes: Attribute[],
  fields: Record<string, string>,
): NewAttributeValue[] {
  return attributes.flatMap<NewAttributeValue>(
    (attribute): NewAttributeValue[] => {
      const value = fields[attribute.code]?.trim()
      if (!value) return []
      if (attribute.value_type === 'relationship') return []
      return [
        { kind: 'scalar' as const, attribute_code: attribute.code, value },
      ]
    },
  )
}

export function relationshipTargetsForForm(
  attributes: Attribute[],
  fields: Record<string, string>,
): RelationshipTargets[] {
  return attributes
    .filter((attribute) => attribute.value_type === 'relationship')
    .map((attribute) => ({
      attribute_code: attribute.code,
      target_entity_ids: (fields[attribute.code] ?? '')
        .split(',')
        .map((targetEntityId) => targetEntityId.trim())
        .filter(Boolean),
    }))
}

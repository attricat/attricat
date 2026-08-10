import { describe, expect, it } from 'vitest'
import {
  relationshipTargetsForForm,
  serializeAttributeValues,
  valuesForForm,
} from './entity-form'

const attributes = [
  { code: 'title', value_type: 'string', tags: ['display'] },
  { code: 'related_products', value_type: 'relationship', tags: [] },
]

describe('entity form values', () => {
  it('serializes scalar fields and complete relationship target sets', () => {
    expect(
      serializeAttributeValues(attributes, {
        title: 'Summer shirt',
        related_products: 'uuid-one, uuid-two , ',
      }),
    ).toEqual([
      { kind: 'scalar', attribute_code: 'title', value: 'Summer shirt' },
    ])
    expect(
      relationshipTargetsForForm(attributes, {
        related_products: 'uuid-one, uuid-two , ',
      }),
    ).toEqual([
      {
        attribute_code: 'related_products',
        target_entity_ids: ['uuid-one', 'uuid-two'],
      },
    ])
  })

  it('hydrates relationship values into a comma-separated field', () => {
    expect(
      valuesForForm(attributes, [
        { kind: 'scalar', attribute_code: 'title', value: 'Summer shirt' },
        {
          kind: 'relationship',
          attribute_code: 'related_products',
          target_entity_id: 'uuid-one',
        },
        {
          kind: 'relationship',
          attribute_code: 'related_products',
          target_entity_id: 'uuid-two',
        },
      ]),
    ).toEqual({ title: 'Summer shirt', related_products: 'uuid-one, uuid-two' })
  })
})

import { describe, expect, it } from 'vitest'
import { displayValue, parseExplorerSearch, previewHeading } from './search'

describe('parseExplorerSearch', () => {
  it('retains valid URL state and drops invalid version values', () => {
    expect(
      parseExplorerSearch({
        blueprint: 'product',
        version: '3',
        query: 'shirt',
      }),
    ).toEqual({ blueprint: 'product', version: 3, query: 'shirt' })
    expect(parseExplorerSearch({ blueprint: '', version: 'zero' })).toEqual({
      blueprint: undefined,
      version: undefined,
      query: undefined,
    })
  })
})

describe('previewHeading', () => {
  it('uses the first display-tagged default value and falls back to the entity ID', () => {
    const attributes = [
      { code: 'sku', value_type: 'string', tags: [] },
      { code: 'title', value_type: 'string', tags: ['display'] },
      { code: 'subtitle', value_type: 'string', tags: ['display'] },
    ]
    expect(
      previewHeading(
        { default: { title: 'Summer shirt' } },
        attributes,
        'entity-id',
      ),
    ).toBe('Summer shirt')
    expect(previewHeading({ default: {} }, attributes, 'entity-id')).toBe(
      'entity-id',
    )
  })
})

describe('displayValue', () => {
  it('formats scalar, empty, and structured preview values', () => {
    expect(displayValue(null)).toBe('—')
    expect(displayValue({ amount: 12 })).toBe('{"amount":12}')
  })
})

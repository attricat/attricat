import { describe, expect, it } from 'vitest'
import { displayLabel, parseExplorerSearch } from './search'

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

describe('displayLabel', () => {
  it('uses the backend default label and falls back to the entity ID', () => {
    expect(displayLabel({ default: 'Summer shirt' }, 'entity-id')).toBe(
      'Summer shirt',
    )
    expect(displayLabel({}, 'entity-id')).toBe('entity-id')
  })
})

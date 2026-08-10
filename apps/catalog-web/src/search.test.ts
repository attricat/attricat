import { describe, expect, it } from 'vitest'
import { displayValue, parseExplorerSearch } from './search'

describe('parseExplorerSearch', () => {
  it('retains valid URL state and drops invalid version values', () => {
    expect(parseExplorerSearch({ blueprint: 'product', version: '3', query: 'shirt' })).toEqual({ blueprint: 'product', version: 3, query: 'shirt' })
    expect(parseExplorerSearch({ blueprint: '', version: 'zero' })).toEqual({ blueprint: undefined, version: undefined, query: undefined })
  })
})

describe('displayValue', () => {
  it('formats scalar, empty, and structured preview values', () => {
    expect(displayValue(null)).toBe('—')
    expect(displayValue({ amount: 12 })).toBe('{"amount":12}')
  })
})

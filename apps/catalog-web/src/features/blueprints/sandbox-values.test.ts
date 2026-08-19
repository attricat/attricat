import { describe, expect, it } from 'vitest';
import { sandboxValuesForFields } from './sandbox-values';

const attribute = {
  code: 'price',
  value_type: 'number' as const,
  value_schema: null,
};

describe('sandboxValuesForFields', () => {
  it('exposes only valid typed input values to view previews', () => {
    expect(sandboxValuesForFields([attribute], { price: '12.50' })).toEqual({
      price: { value: 12.5 },
    });
    expect(
      sandboxValuesForFields([attribute], { price: 'not a number' }),
    ).toEqual({});
  });
});

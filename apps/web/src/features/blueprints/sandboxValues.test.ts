import { describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { sandboxValidation, sandboxValuesForFields } from './sandboxValues';

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

describe('sandboxValidation', () => {
  const size = {
    code: 'size',
    value_type: 'string' as const,
    value_schema: { type: 'string', enum: ['small', 'large'] },
  };

  it('reports nothing until a field is filled in', () => {
    expect(
      sandboxValidation([attribute, size], {}, { required: ['price'] }),
    ).toEqual({ fieldErrors: {} });
  });

  it('reports value schema errors only for filled-in fields', () => {
    expect(
      sandboxValidation(
        [attribute, size],
        { size: 'medium' },
        {
          required: ['price'],
        },
      ).fieldErrors,
    ).toEqual({ size: i18n.t('records.schemaErrors.enum') });
  });

  it('reports record schema errors once values are valid', () => {
    expect(
      sandboxValidation(
        [attribute, size],
        { size: 'small' },
        {
          required: ['price'],
        },
      ),
    ).toMatchObject({ fieldErrors: {} });
    expect(
      sandboxValidation(
        [attribute, size],
        { price: '5' },
        {
          properties: { price: { maximum: 3 } },
        },
      ).fieldErrors,
    ).toEqual({
      price: i18n.t('records.schemaErrors.maximum', { limit: '3' }),
    });
  });
});

import type { TFunction } from 'i18next';
import type { Attribute } from '../entities/api';
import type { AttributeFilter } from './search';

export type AttributeFilterOperator = AttributeFilter['operator'];

export const filterableValueTypes = [
  'string',
  'number',
  'integer',
  'boolean',
  'date',
  'datetime',
  'time',
] as const;

export const isFilterableAttribute = (attribute: Attribute) =>
  filterableValueTypes.some((valueType) => valueType === attribute.value_type);

export const operatorsForValueType = (
  valueType: Attribute['value_type'],
): AttributeFilterOperator[] => {
  if (valueType === 'string') return ['eq', 'contains', 'starts_with'];
  if (valueType === 'boolean') return ['eq'];
  if (
    valueType === 'number' ||
    valueType === 'integer' ||
    valueType === 'date' ||
    valueType === 'datetime' ||
    valueType === 'time'
  )
    return ['eq', 'gt', 'gte', 'lt', 'lte'];
  return [];
};

/**
 * Attribute filters have no identity of their own and may repeat, so their
 * position is part of the key.
 */
export const attributeFilterKey = (filter: AttributeFilter, index: number) =>
  `${filter.field}-${filter.operator}-${String(filter.value)}-${index}`;

export const attributeFilterValueLabel = (
  t: TFunction,
  filter: AttributeFilter,
) =>
  typeof filter.value === 'boolean'
    ? t(filter.value ? 'explorer.true' : 'explorer.false')
    : String(filter.value);

export const attributeFilterLabel = (
  t: TFunction,
  filter: AttributeFilter,
  fieldLabel: string = filter.field,
) =>
  t('explorer.attributeFilterPill', {
    field: fieldLabel,
    operator: t(`explorer.filterOperatorSymbols.${filter.operator}`),
    value: JSON.stringify(attributeFilterValueLabel(t, filter)),
  });

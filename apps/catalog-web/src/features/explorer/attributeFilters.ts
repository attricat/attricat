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

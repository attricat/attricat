import type { TFunction } from 'i18next';
import type { Attribute } from '../entities/api';
import { statusConfiguration, statusLabel } from '../entities/status';
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

/** Status codes are matched exactly, so a status offers equality only. */
export const operatorsForAttribute = (
  attribute: Attribute | undefined,
): AttributeFilterOperator[] =>
  attribute && statusConfiguration(attribute)
    ? ['eq']
    : operatorsForValueType(attribute?.value_type ?? 'string');

/**
 * Attribute filters have no identity of their own and may repeat, so their
 * position is part of the key.
 */
export const attributeFilterKey = (filter: AttributeFilter, index: number) =>
  `${filter.field}-${filter.operator}-${String(filter.value)}-${index}`;

/** The filter value as shown to the user; status codes show their label. */
export const attributeFilterValueLabel = (
  t: TFunction,
  filter: AttributeFilter,
  attribute?: Attribute,
) =>
  typeof filter.value === 'boolean'
    ? t(filter.value ? 'explorer.true' : 'explorer.false')
    : ((attribute && statusLabel(attribute, filter.value)) ??
      String(filter.value));

export const attributeFilterLabel = (
  t: TFunction,
  filter: AttributeFilter,
  fieldLabel: string = filter.field,
  attribute?: Attribute,
) =>
  t('explorer.attributeFilterPill', {
    field: fieldLabel,
    operator: t(`explorer.filterOperatorSymbols.${filter.operator}`),
    value: JSON.stringify(attributeFilterValueLabel(t, filter, attribute)),
  });

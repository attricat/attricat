import type { TFunction } from 'i18next';
import type { Attribute } from '../entities/api';
import { attributeLabel } from '../entities/entityDisplay';
import { statusConfiguration, statusLabel } from '../entities/status';
import { CURRENT_USER_FILTER_VALUE } from '../principals/constants';
import {
  principalConfiguration,
  resolvePrincipal,
} from '../principals/principal';
import type { Directory } from '../principals/schemas';
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
 * Status codes and user-or-team references are matched exactly, so they offer
 * equality only.
 */
export const operatorsForAttribute = (
  attribute: Attribute | undefined,
): AttributeFilterOperator[] =>
  attribute &&
  (statusConfiguration(attribute) || principalConfiguration(attribute))
    ? ['eq']
    : operatorsForValueType(attribute?.value_type ?? 'string');

/**
 * Attribute filters have no identity of their own and may repeat, so their
 * position is part of the key.
 */
export const attributeFilterKey = (filter: AttributeFilter, index: number) =>
  `${filter.field}-${filter.operator}-${String(filter.value)}-${index}`;

const principalFilterValueLabel = (
  t: TFunction,
  value: unknown,
  directory: Directory | undefined,
) =>
  value === CURRENT_USER_FILTER_VALUE
    ? t('explorer.assignedToMe')
    : resolvePrincipal(directory, value)?.label;

/**
 * The filter value as shown to the user: status codes show their label, and
 * user-or-team references the name from `directory`.
 */
export const attributeFilterValueLabel = (
  t: TFunction,
  filter: AttributeFilter,
  attribute?: Attribute,
  directory?: Directory,
) =>
  typeof filter.value === 'boolean'
    ? t(filter.value ? 'explorer.true' : 'explorer.false')
    : ((attribute &&
        (statusLabel(attribute, filter.value) ??
          (principalConfiguration(attribute)
            ? principalFilterValueLabel(t, filter.value, directory)
            : undefined))) ??
      String(filter.value));

/** A filter as one pill label; `attribute` names the field and its values. */
export const attributeFilterLabel = (
  t: TFunction,
  filter: AttributeFilter,
  attribute?: Attribute,
  directory?: Directory,
) =>
  t('explorer.attributeFilterPill', {
    field: attributeLabel(attribute ?? { code: filter.field }),
    operator: t(`explorer.filterOperatorSymbols.${filter.operator}`),
    value: JSON.stringify(
      attributeFilterValueLabel(t, filter, attribute, directory),
    ),
  });

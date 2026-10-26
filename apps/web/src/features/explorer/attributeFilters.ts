import type { TFunction } from 'i18next';
import type { Attribute } from '../records/api';
import { attributeFilterOperators } from '../records/constants';
import { attributeValueTypes } from '../records/valueTypes';
import { attributeLabel } from '../records/recordDisplay';
import { choiceOptions, choiceValueLabel } from '../records/choices';
import { statusConfiguration, statusLabel } from '../records/status';
import { CURRENT_USER_FILTER_VALUE } from '../principals/constants';
import {
  principalConfiguration,
  resolvePrincipal,
} from '../principals/principal';
import type { Directory } from '../principals/schemas';
import type { AttributeFilter } from './search';

export type AttributeFilterOperator = AttributeFilter['operator'];

export const filterableValueTypes = [
  attributeValueTypes.string,
  attributeValueTypes.number,
  attributeValueTypes.integer,
  attributeValueTypes.boolean,
  attributeValueTypes.date,
  attributeValueTypes.datetime,
  attributeValueTypes.time,
] as const;

/** File values can only be filtered by whether any file is attached. */
const presenceOnlyValueTypes = [attributeValueTypes.file] as const;

const { contains, eq, gt, gte, isSet, lt, lte, startsWith } =
  attributeFilterOperators;
/** Operators of values ordered by size or time. */
const comparisonOperators = [eq, gt, gte, lt, lte, isSet];

export const isFilterableAttribute = (attribute: Attribute) =>
  [...filterableValueTypes, ...presenceOnlyValueTypes].some(
    (valueType) => valueType === attribute.value_type,
  );

export const operatorsForValueType = (
  valueType: Attribute['value_type'],
): AttributeFilterOperator[] => {
  switch (valueType) {
    case attributeValueTypes.string:
      return [eq, contains, startsWith, isSet];
    case attributeValueTypes.boolean:
      return [eq, isSet];
    case attributeValueTypes.file:
      return [isSet];
    case attributeValueTypes.number:
    case attributeValueTypes.integer:
    case attributeValueTypes.date:
    case attributeValueTypes.datetime:
    case attributeValueTypes.time:
      return comparisonOperators;
    default:
      return [];
  }
};

/**
 * Status codes, `enum` values and user-or-team references are matched
 * exactly, so they offer equality and presence, not text matching.
 */
export const operatorsForAttribute = (
  attribute: Attribute | undefined,
): AttributeFilterOperator[] =>
  attribute &&
  (statusConfiguration(attribute) ||
    choiceOptions(attribute) ||
    principalConfiguration(attribute))
    ? [eq, isSet]
    : operatorsForValueType(
        attribute?.value_type ?? attributeValueTypes.string,
      );

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
 * The filter value as shown to the user: status codes and `enum` values show
 * their label, and
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
          choiceValueLabel(attribute, filter.value) ??
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
  filter.operator === isSet
    ? t(
        filter.value
          ? 'explorer.attributeFilterIsSet'
          : 'explorer.attributeFilterIsNotSet',
        {
          field: attributeLabel(attribute ?? { code: filter.field }),
        },
      )
    : t('explorer.attributeFilterPill', {
        field: attributeLabel(attribute ?? { code: filter.field }),
        operator: t(`explorer.filterOperatorSymbols.${filter.operator}`),
        value: JSON.stringify(
          attributeFilterValueLabel(t, filter, attribute, directory),
        ),
      });

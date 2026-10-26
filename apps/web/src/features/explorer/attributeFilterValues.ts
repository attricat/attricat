import type { Attribute } from '../records/api';
import { attributeFilterOperators } from '../records/constants';
import type { AttributeFilterOperator } from './attributeFilters';
import { defaultAttributeFilterOperator } from './constants';
import {
  isoToZonedDateTime,
  utcTimeZone,
  zonedDateTimeToIso,
} from '../../time/instantFormat';
import type { AttributeFilter } from './search';
import { attributeValueTypes } from '../records/valueTypes';

type ValueType = Attribute['value_type'] | undefined;

/** String values edited by the attribute filter dialog. */
export type AttributeFilterDraft = {
  field: string;
  operator: AttributeFilterOperator;
  value: string;
};

/**
 * Asks the filter picker to open its dialog with a prepared draft. The id
 * distinguishes repeated requests for the same draft.
 */
export type AttributeFilterRequest = {
  id: number;
  draft: AttributeFilterDraft;
};

export const emptyAttributeFilterDraft: AttributeFilterDraft = {
  field: '',
  operator: defaultAttributeFilterOperator,
  value: '',
};

const integerPattern = /^-?\d+$/;

export const booleanFilterValues = {
  false: 'false',
  true: 'true',
} as const;

/**
 * Converts an edited string into a filter value. Datetime input is wall-clock
 * time in `timeZone`, the user's effective zone.
 */
export const parseAttributeFilterValue = (
  valueType: ValueType,
  value: string,
  timeZone: string,
  operator?: AttributeFilterOperator,
): AttributeFilter['value'] => {
  if (operator === attributeFilterOperators.isSet)
    return value === booleanFilterValues.true;
  if (valueType === attributeValueTypes.number) return Number(value);
  if (valueType === attributeValueTypes.integer)
    return Number.parseInt(value, 10);
  if (valueType === attributeValueTypes.boolean)
    return value === booleanFilterValues.true;
  if (valueType === attributeValueTypes.datetime)
    return zonedDateTimeToIso(value, timeZone);
  return value;
};

export const isAttributeFilterValueValid = (
  valueType: ValueType,
  value: string,
  operator?: AttributeFilterOperator,
) => {
  if (operator === attributeFilterOperators.isSet) {
    return (
      value === booleanFilterValues.true || value === booleanFilterValues.false
    );
  }
  const numericValue = Number(value);
  return (
    value !== '' &&
    (valueType !== attributeValueTypes.number ||
      Number.isFinite(numericValue)) &&
    (valueType !== attributeValueTypes.integer ||
      (integerPattern.test(value) && Number.isSafeInteger(numericValue))) &&
    (valueType !== attributeValueTypes.datetime ||
      zonedDateTimeToIso(value, utcTimeZone) !== '')
  );
};

export const attributeFilterInputType = (valueType: ValueType) =>
  valueType === attributeValueTypes.number ||
  valueType === attributeValueTypes.integer
    ? 'number'
    : valueType === attributeValueTypes.date
      ? 'date'
      : valueType === attributeValueTypes.datetime
        ? 'datetime-local'
        : valueType === attributeValueTypes.time
          ? 'time'
          : 'text';

/**
 * Converts a stored filter value into the string an input control edits;
 * datetime values become wall-clock time in `timeZone`.
 */
export const attributeFilterInputValue = (
  filter: AttributeFilter,
  valueType: ValueType,
  timeZone: string,
) => {
  const inputValue = String(filter.value);
  return valueType === attributeValueTypes.datetime &&
    filter.operator !== attributeFilterOperators.isSet
    ? isoToZonedDateTime(inputValue, timeZone)
    : inputValue;
};

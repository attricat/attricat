import type { Attribute } from '../entities/api';
import type { AttributeFilterOperator } from './attributeFilters';
import {
  datetimeLocalInputLength,
  defaultAttributeFilterOperator,
  millisecondsPerMinute,
} from './constants';
import type { AttributeFilter } from './search';

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

export const parseAttributeFilterValue = (
  valueType: ValueType,
  value: string,
): AttributeFilter['value'] => {
  if (valueType === 'number') return Number(value);
  if (valueType === 'integer') return Number.parseInt(value, 10);
  if (valueType === 'boolean') return value === booleanFilterValues.true;
  if (valueType === 'datetime') return new Date(value).toISOString();
  return value;
};

export const isAttributeFilterValueValid = (
  valueType: ValueType,
  value: string,
) => {
  const numericValue = Number(value);
  return (
    value !== '' &&
    (valueType !== 'number' || Number.isFinite(numericValue)) &&
    (valueType !== 'integer' ||
      (integerPattern.test(value) && Number.isSafeInteger(numericValue))) &&
    (valueType !== 'datetime' || !Number.isNaN(new Date(value).getTime()))
  );
};

export const attributeFilterInputType = (valueType: ValueType) =>
  valueType === 'number' || valueType === 'integer'
    ? 'number'
    : valueType === 'date'
      ? 'date'
      : valueType === 'datetime'
        ? 'datetime-local'
        : valueType === 'time'
          ? 'time'
          : 'text';

/** Converts a stored filter value into the string an input control edits. */
export const attributeFilterInputValue = (
  filter: AttributeFilter,
  valueType: ValueType,
) => {
  const inputValue = String(filter.value);
  if (valueType !== 'datetime') return inputValue;
  const date = new Date(inputValue);
  return new Date(
    date.getTime() - date.getTimezoneOffset() * millisecondsPerMinute,
  )
    .toISOString()
    .slice(0, datetimeLocalInputLength);
};

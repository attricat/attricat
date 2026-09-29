import type { Attribute } from '../entities/api';
import { filterableValueTypes } from './attributeFilters';
import {
  attributeFilterInputValue,
  type AttributeFilterDraft,
} from './attributeFilterValues';
import { cellFilterOperator, relationshipPathSeparator } from './constants';
import type { AttributeFilter } from './search';

type ValueType = Attribute['value_type'];

const isFilterableValueType = (valueType: ValueType | undefined) =>
  filterableValueTypes.some((filterable) => filterable === valueType);

/**
 * Resolves the value type a column can be filtered by: the attribute itself
 * for direct fields, or the projected path attribute for relationship paths.
 */
export const cellFilterValueType = (
  field: string,
  attribute: Attribute,
  pathAttributes: { code: string; value_type: ValueType }[],
): ValueType | undefined => {
  const valueType = field.includes(relationshipPathSeparator)
    ? pathAttributes.find((path) => path.code === field)?.value_type
    : attribute.value_type;
  return isFilterableValueType(valueType) ? valueType : undefined;
};

const matchesValueType = (
  valueType: ValueType,
  value: unknown,
): value is AttributeFilter['value'] =>
  valueType === 'number' || valueType === 'integer'
    ? typeof value === 'number' && Number.isFinite(value)
    : valueType === 'boolean'
      ? typeof value === 'boolean'
      : typeof value === 'string' && value !== '';

/** Builds an equality filter for a single scalar cell value, if it has one. */
export const cellValueFilter = (
  field: string,
  valueType: ValueType,
  cellValues: unknown[],
): AttributeFilter | undefined => {
  const [value] = cellValues;
  return cellValues.length === 1 && matchesValueType(valueType, value)
    ? { field, operator: cellFilterOperator, value }
    : undefined;
};

/** Converts a cell filter into the draft the filter dialog edits. */
export const cellFilterDraft = (
  filter: AttributeFilter,
  valueType: ValueType,
): AttributeFilterDraft => ({
  field: filter.field,
  operator: filter.operator,
  value: attributeFilterInputValue(filter, valueType),
});

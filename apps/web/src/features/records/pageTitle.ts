import { formatAttributeValue } from '../views/components/values/formatAttributeValue';
import type { Attribute } from './api';
import { fieldListViewLabel } from './recordDisplay';
import { viewBlockTypes } from './schemas';

/**
 * The record page title from the blueprint's `page_title` view and the
 * record's resolved values, or `null` when the view is missing or its fields
 * are all empty.
 */
export const pageTitleLabel = (
  views: Record<string, unknown>,
  attributes: readonly Attribute[],
  values: Record<string, { value: unknown }>,
): string | null =>
  fieldListViewLabel(views[viewBlockTypes.pageTitle], (field) => {
    const attribute = attributes.find((item) => item.code === field);
    const value = values[field]?.value;
    return attribute && value !== null && value !== undefined
      ? formatAttributeValue(attribute, value)
      : undefined;
  }) ?? null;

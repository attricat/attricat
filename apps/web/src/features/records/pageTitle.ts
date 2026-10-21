import { z } from 'zod';
import { formatAttributeValue } from '../views/components/values/formatAttributeValue';
import type { Attribute } from './api';

const pageTitleSchema = z.object({
  fields: z.array(z.string()),
  separator: z.string().optional(),
});

/**
 * The record page title from the blueprint's `page_title` view and the
 * record's resolved values, or `null` when the view is missing or its fields
 * are all empty.
 */
export const pageTitleLabel = (
  views: Record<string, unknown>,
  attributes: readonly Attribute[],
  values: Record<string, { value: unknown }>,
): string | null => {
  const definition = pageTitleSchema.safeParse(views.page_title);
  if (!definition.success) return null;
  const label = definition.data.fields
    .flatMap((field) => {
      const attribute = attributes.find((item) => item.code === field);
      const value = values[field]?.value;
      if (!attribute || value === null || value === undefined) return [];
      return [formatAttributeValue(attribute, value)];
    })
    .join(definition.data.separator ?? ' · ');
  return label || null;
};

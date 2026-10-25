import { z } from 'zod';
import { lexiconText } from '../lexicon/lexicon';
import { fieldListViewSchema, viewBlockTypes } from './schemas';

const previewSchema = z.record(z.string(), z.record(z.string(), z.unknown()));
const defaultViewFieldSeparator = ' · ';

/**
 * The label a `dropdown_option` or `page_title` view builds: the text of each
 * listed field joined by the view's separator, skipping empty fields.
 * `undefined` when the view is missing or every field is empty.
 */
export const fieldListViewLabel = (
  view: unknown,
  fieldText: (field: string) => string | undefined,
) => {
  const definition = fieldListViewSchema.safeParse(view);
  if (!definition.success) return undefined;
  const label = definition.data.fields
    .map(fieldText)
    .filter((text): text is string => Boolean(text?.trim()))
    .join(definition.data.separator ?? defaultViewFieldSeparator);
  return label || undefined;
};

export const attributeLabel = (attribute: {
  code: string;
  name?: unknown;
}): string => {
  if (typeof attribute.name === 'string' && attribute.name.trim())
    return lexiconText(attribute.name);
  return attribute.code.replaceAll('_', ' ');
};

export const displayLabel = (
  display: Record<string, string> | undefined,
  recordId: string,
): string => {
  return display?.default || recordId;
};

/**
 * The label of the first context in `contextCodes` (a context followed by
 * its ancestors) that has one.
 */
export const contextDisplayLabel = (
  display: Record<string, string> | undefined,
  recordId: string,
  contextCodes: readonly string[],
): string =>
  contextCodes.map((code) => display?.[code]).find(Boolean) ||
  displayLabel(display, recordId);

export const dropdownOptionLabel = (
  preview: Record<string, unknown>,
  views: Record<string, unknown>,
): string | undefined => {
  const values = previewSchema.safeParse(preview).data?.default;
  if (!values) return undefined;
  return fieldListViewLabel(views[viewBlockTypes.dropdownOption], (field) => {
    const value = values[field];
    return value === null || value === undefined ? undefined : String(value);
  });
};

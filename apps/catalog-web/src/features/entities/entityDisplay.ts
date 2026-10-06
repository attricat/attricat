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
  entityId: string,
): string => {
  return display?.default || entityId;
};

/**
 * The label of the first context in `contextCodes` (a context followed by
 * its ancestors) that has one.
 */
export const contextDisplayLabel = (
  display: Record<string, string> | undefined,
  entityId: string,
  contextCodes: readonly string[],
): string =>
  contextCodes.map((code) => display?.[code]).find(Boolean) ||
  displayLabel(display, entityId);

export const dropdownOptionLabel = (
  preview: Record<string, unknown>,
  views: Record<string, unknown>,
): string | undefined => {
  const definition = dropdownOptionSchema.safeParse(views.dropdown_option);
  const parsedPreview = previewSchema.safeParse(preview);
  if (!definition.success || !parsedPreview.success) return undefined;
  const values = parsedPreview.data.default;
  if (!values) return undefined;
  const label = definition.data.fields
    .map((field) => values[field])
    .filter((value) => value !== null && value !== undefined)
    .map((value) => (typeof value === 'string' ? value : String(value)))
    .join(definition.data.separator ?? ' · ');
  return label || undefined;
};
import { z } from 'zod';
import { lexiconText } from '../lexicon/lexicon';

const dropdownOptionSchema = z.object({
  fields: z.array(z.string()),
  separator: z.string().optional(),
});
const previewSchema = z.record(z.string(), z.record(z.string(), z.unknown()));

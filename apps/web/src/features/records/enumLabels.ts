import { lexiconText } from '../lexicon/lexicon';

export const ENUM_LABELS_SCHEMA_KEY = 'x-attricat-enum-labels';

/**
 * The label `x-attricat-enum-labels` gives one `enum` value of a value
 * schema, if any. A light lookup for places that only name values.
 */
export const enumValueLabel = (
  schema: unknown,
  code: string,
): string | undefined => {
  if (!schema || typeof schema !== 'object') return undefined;
  const labels = (schema as Record<string, unknown>)[ENUM_LABELS_SCHEMA_KEY];
  const label =
    labels && typeof labels === 'object' && !Array.isArray(labels)
      ? (labels as Record<string, unknown>)[code]
      : undefined;
  return typeof label === 'string' && label.trim()
    ? lexiconText(label)
    : undefined;
};

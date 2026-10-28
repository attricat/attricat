import type { Attribute } from '../records/api';
import { enumValueLabel } from '../records/enumLabels';
import { attributeLabel } from '../records/recordDisplay';
import { statusLabel } from '../records/status';

/** An attribute a finding's message may name, with its options when known. */
export type FindingAttribute = { code: string; name?: unknown } & Partial<
  Pick<Attribute, 'value_type' | 'value_schema'>
>;

/** Attribute codes as messages quote them: `'listing_status'`. */
const QUOTED_CODE = /'([A-Za-z0-9_-]+)'/g;
/** Values as messages quote them: `"approved"`. */
const QUOTED_VALUE = /"((?:[^"\\]|\\.)*)"/g;

/** The label of one of an attribute's fixed values, such as a status. */
const optionLabel = (attribute: FindingAttribute, value: string) => {
  if (attribute.value_type === undefined) return undefined;
  const full = attribute as Attribute;
  return statusLabel(full, value) ?? enumValueLabel(full.value_schema, value);
};

/**
 * Words a finding's message with labels instead of codes: each attribute it
 * quotes, and the fixed values quoted after it, such as the allowed
 * statuses, as their option labels. Unknown codes stay as they are.
 */
export const findingMessage = (
  message: string,
  attributes: readonly FindingAttribute[],
) => {
  if (attributes.length === 0) return message;
  const byCode = new Map(attributes.map((item) => [item.code, item]));
  // Each part starts at the attribute it is about; values follow it.
  let current: FindingAttribute | undefined;
  let result = '';
  let last = 0;
  for (const match of message.matchAll(QUOTED_CODE)) {
    result += labelValues(message.slice(last, match.index), current);
    current = byCode.get(match[1]!);
    result += current ? attributeLabel(current) : match[0];
    last = match.index + match[0].length;
  }
  return result + labelValues(message.slice(last), current);
};

const labelValues = (text: string, attribute: FindingAttribute | undefined) =>
  attribute
    ? text.replaceAll(QUOTED_VALUE, (quoted, value: string) => {
        const label = optionLabel(attribute, value);
        return label ? `"${label}"` : quoted;
      })
    : text;

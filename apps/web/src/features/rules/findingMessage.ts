import i18n from '../../i18n';
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

/** What a comparison's message says in place of each operator. */
const COMPARE_PHRASES = {
  'equal to': 'eq',
  'different from': 'ne',
  'less than': 'lt',
  'at most': 'lte',
  'greater than': 'gt',
  'at least': 'gte',
  'without common targets with': 'disjoint',
} as const;

const operand = (text: string) => {
  const daysFromNow = /^(\d+) days from now$/.exec(text);
  const daysAgo = /^(\d+) days ago$/.exec(text);
  if (text === 'now') return i18n.t('rules.findingMessages.now');
  if (daysFromNow)
    return i18n.t('rules.findingMessages.daysFromNow', {
      days: daysFromNow[1],
    });
  if (daysAgo)
    return i18n.t('rules.findingMessages.daysAgo', { days: daysAgo[1] });
  return text;
};

const compareMessage = (attribute: string, phrase: string, other: string) =>
  i18n.t('rules.findingMessages.compare', {
    attribute,
    op: i18n.t(
      `rules.findingMessages.ops.${COMPARE_PHRASES[phrase as keyof typeof COMPARE_PHRASES]}`,
    ),
    other: operand(other),
  });

/** The server words findings in English: each shape it words, as a sentence
 * in the user's language that still quotes codes and values. */
const MESSAGE_SHAPES: readonly [RegExp, (...parts: string[]) => string][] = [
  [
    /^None of the alternatives hold: (.+)$/s,
    (alternatives) =>
      i18n.t('rules.findingMessages.noneHold', {
        alternatives: alternatives!
          .split('; or ')
          .map(localizedMessage)
          .join(i18n.t('rules.findingMessages.alternativeSeparator')),
      }),
  ],
  [
    /^Every ('[^']+') record must satisfy the check: (.+)$/s,
    (relationship, check) =>
      i18n.t('rules.findingMessages.everyLinked', {
        relationship,
        check: localizedMessage(check!),
      }),
  ],
  [
    /^At least one ('[^']+') record must satisfy the check: (.+)$/s,
    (relationship, check) =>
      i18n.t('rules.findingMessages.anyLinked', { relationship, check }),
  ],
  [
    /^No ('[^']+') record may satisfy: (.+)$/s,
    (relationship, check) =>
      i18n.t('rules.findingMessages.noLinked', { relationship, check }),
  ],
  [
    /^At least (\d+) ('[^']+') records must reference this record through ('[^']+') \(found (\d+)\)$/,
    (limit, blueprint, relationship, found) =>
      i18n.t('rules.findingMessages.referencedAtLeast', {
        limit,
        blueprint,
        relationship,
        found,
      }),
  ],
  [
    /^At most (\d+) ('[^']+') records may reference this record through ('[^']+') \(found (\d+)\)$/,
    (limit, blueprint, relationship, found) =>
      i18n.t('rules.findingMessages.referencedAtMost', {
        limit,
        blueprint,
        relationship,
        found,
      }),
  ],
  [
    /^No ('[^']+') record may reference this record through ('[^']+') \(found (\d+)\)$/,
    (blueprint, relationship, found) =>
      i18n.t('rules.findingMessages.referencedNone', {
        blueprint,
        relationship,
        found,
      }),
  ],
  [
    /^Another record has the same (.+)$/,
    (attributes) => i18n.t('rules.findingMessages.unique', { attributes }),
  ],
  [
    /^Attribute ('[^']+') is older than (\d+) seconds$/,
    (attribute, seconds) =>
      i18n.t('rules.findingMessages.stale', { attribute, seconds }),
  ],
  [
    /^Record is missing required system tag ('[^']+')$/,
    (tag) => i18n.t('rules.findingMessages.missingTag', { tag }),
  ],
  [
    /^Record has prohibited system tag ('[^']+')$/,
    (tag) => i18n.t('rules.findingMessages.prohibitedTag', { tag }),
  ],
  [
    /^('[^']+') is required$/,
    (attribute) => i18n.t('rules.findingMessages.required', { attribute }),
  ],
  [
    /^('[^']+') must be one of (.+)$/s,
    (attribute, values) =>
      i18n.t('rules.findingMessages.oneOf', { attribute, values }),
  ],
  [
    new RegExp(
      `^('[^']+') must be (${Object.keys(COMPARE_PHRASES).join('|')}) (.+)$`,
      's',
    ),
    (attribute, phrase, other) => compareMessage(attribute!, phrase!, other!),
  ],
  [
    /^('[^']+') cannot be compared with (.+)$/s,
    (attribute, other) =>
      i18n.t('rules.findingMessages.cannotCompare', { attribute, other }),
  ],
  [
    /^('[^']+') forms a cycle$/,
    (relationship) => i18n.t('rules.findingMessages.cycle', { relationship }),
  ],
];

function localizedMessage(message: string): string {
  for (const [shape, word] of MESSAGE_SHAPES) {
    const match = shape.exec(message);
    if (match) return word(...match.slice(1));
  }
  return message;
}

/** The label of one of an attribute's fixed values, such as a status. */
export const optionLabel = (attribute: FindingAttribute, value: string) => {
  if (attribute.value_type === undefined) return undefined;
  const full = attribute as Attribute;
  return statusLabel(full, value) ?? enumValueLabel(full.value_schema, value);
};

/**
 * Words a finding's message in the user's language, with labels instead of
 * codes: each attribute it
 * quotes, and the fixed values quoted after it, such as the allowed
 * statuses, as their option labels. Unknown codes stay as they are.
 */
export const findingMessage = (
  serverMessage: string,
  attributes: readonly FindingAttribute[],
) => {
  const message = localizedMessage(serverMessage);
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

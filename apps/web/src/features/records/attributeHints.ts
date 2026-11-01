import type { TFunction } from 'i18next';
import { formatBytes } from '../data-health/dataHealthFormat';
import { lexiconText } from '../lexicon/lexicon';
import type { Attribute } from './api';
import { attributeCardinalities } from './constants';
import { patternHint } from './jsonSchema';
import { attributeValueTypes } from './valueTypes';
import { limitNumberFormat } from './limitNumberFormat';

const schemaNumber = (attribute: Attribute, key: string) => {
  const schema = attribute.value_schema;
  if (!schema || typeof schema !== 'object') return undefined;
  const value = (schema as Record<string, unknown>)[key];
  return typeof value === 'number' && Number.isFinite(value)
    ? value
    : undefined;
};

/** The longest text the attribute's value schema allows, if it says. */
export const attributeMaxLength = (attribute: Attribute) =>
  attribute.value_type === attributeValueTypes.string
    ? schemaNumber(attribute, 'maxLength')
    : undefined;

const lengthHint = (attribute: Attribute, t: TFunction) => {
  const min = schemaNumber(attribute, 'minLength');
  const max = attributeMaxLength(attribute);
  if (min !== undefined && max !== undefined)
    return t('records.hints.lengthBetween', { min, max });
  if (max !== undefined) return t('records.hints.lengthAtMost', { count: max });
  if (min !== undefined)
    return t('records.hints.lengthAtLeast', { count: min });
  return undefined;
};

/** The shape a text value must take: its pattern hint, or its format. */
const shapeHint = (attribute: Attribute, t: TFunction) => {
  if (attribute.value_type !== attributeValueTypes.string) return undefined;
  const hint = patternHint(attribute.value_schema);
  if (hint) return /[.!?]$/.test(hint) ? hint : `${hint}.`;
  const schema = attribute.value_schema;
  const format =
    schema && typeof schema === 'object'
      ? (schema as Record<string, unknown>).format
      : undefined;
  if (format === 'email') return t('records.hints.email');
  if (format === 'uri' || format === 'url') return t('records.hints.url');
  return undefined;
};

/**
 * The lower and upper bounds of a number, each inclusive or exclusive. Where
 * a schema gives both kinds of one bound, the stricter one applies.
 */
const bound = (
  attribute: Attribute,
  inclusiveKey: string,
  exclusiveKey: string,
  stricter: (a: number, b: number) => boolean,
) => {
  const inclusive = schemaNumber(attribute, inclusiveKey);
  const exclusive = schemaNumber(attribute, exclusiveKey);
  if (
    exclusive !== undefined &&
    (inclusive === undefined || !stricter(inclusive, exclusive))
  )
    return { value: exclusive, exclusive: true };
  return inclusive === undefined
    ? undefined
    : { value: inclusive, exclusive: false };
};

const rangeHint = (attribute: Attribute, t: TFunction) => {
  const number = limitNumberFormat();
  const lower = bound(
    attribute,
    'minimum',
    'exclusiveMinimum',
    (a, b) => a > b,
  );
  const upper = bound(
    attribute,
    'maximum',
    'exclusiveMaximum',
    (a, b) => a < b,
  );
  const whole = attribute.value_type === attributeValueTypes.integer;
  const lowerKey = lower ? (lower.exclusive ? 'Above' : 'AtLeast') : '';
  const upperKey = upper ? (upper.exclusive ? 'Below' : 'AtMost') : '';
  const kind = whole ? 'Whole' : '';
  // `rangeBetween` predates the open-ended bounds and names the common case.
  const shape =
    lowerKey === 'AtLeast' && upperKey === 'AtMost'
      ? 'Between'
      : `${lowerKey}${upperKey}`;
  const range = shape
    ? t(`records.hints.range${kind}${shape}`, {
        min: lower && number.format(lower.value),
        max: upper && number.format(upper.value),
      })
    : whole
      ? t('records.hints.wholeNumber')
      : undefined;
  const step = schemaNumber(attribute, 'multipleOf');
  const steps =
    step !== undefined && !(whole && step === 1)
      ? t('records.hints.multipleOf', { step: number.format(step) })
      : undefined;
  return [range, steps].filter(Boolean).join(' ') || undefined;
};

const fileHint = (attribute: Attribute, t: TFunction) => {
  const policy = attribute.file_policy;
  if (!policy) return undefined;
  const types = policy.allowed_extensions.map((extension) =>
    extension.replace(/^\./, '').toUpperCase(),
  );
  const hint = [
    types.length > 0
      ? types.join(', ')
      : policy.image_only
        ? t('records.hints.imagesOnly')
        : undefined,
    policy.max_bytes
      ? t('records.hints.fileSize', { size: formatBytes(policy.max_bytes) })
      : undefined,
    policy.cardinality === attributeCardinalities.many
      ? policy.ordered
        ? t('records.hints.filesOrdered')
        : t('records.hints.filesMany')
      : t('records.hints.fileOne'),
  ]
    .filter(Boolean)
    .join(' · ');
  return `${hint.charAt(0).toUpperCase()}${hint.slice(1)}.`;
};

/**
 * What a field holds and accepts, for its helper text: the description its
 * blueprint gives it, then the limits of its value schema or file policy.
 */
export const attributeHint = (attribute: Attribute, t: TFunction) => {
  const rules =
    attribute.value_type === attributeValueTypes.file
      ? fileHint(attribute, t)
      : attribute.value_type === attributeValueTypes.number ||
          attribute.value_type === attributeValueTypes.integer
        ? rangeHint(attribute, t)
        : [lengthHint(attribute, t), shapeHint(attribute, t)]
            .filter(Boolean)
            .join(' ') || undefined;
  return (
    [
      attribute.description ? lexiconText(attribute.description) : undefined,
      rules,
    ]
      .filter(Boolean)
      .join(' ') || undefined
  );
};

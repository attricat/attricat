import type { TFunction } from 'i18next';
import i18n from '../../i18n';
import { formatBytes } from '../data-health/dataHealthFormat';
import { lexiconText } from '../lexicon/lexicon';
import type { Attribute } from './api';
import { attributeValueTypes } from './valueTypes';

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

const rangeHint = (attribute: Attribute, t: TFunction) => {
  const number = new Intl.NumberFormat(i18n.language);
  const minimum = schemaNumber(attribute, 'minimum');
  const maximum = schemaNumber(attribute, 'maximum');
  const min = minimum === undefined ? undefined : number.format(minimum);
  const max = maximum === undefined ? undefined : number.format(maximum);
  const whole = attribute.value_type === attributeValueTypes.integer;
  const kind = whole ? 'Whole' : '';
  if (min !== undefined && max !== undefined)
    return t(`records.hints.range${kind}Between`, { min, max });
  if (min !== undefined) return t(`records.hints.range${kind}AtLeast`, { min });
  if (max !== undefined) return t(`records.hints.range${kind}AtMost`, { max });
  return whole ? t('records.hints.wholeNumber') : undefined;
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
    policy.cardinality === 'many'
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
        : lengthHint(attribute, t);
  return (
    [
      attribute.description ? lexiconText(attribute.description) : undefined,
      rules,
    ]
      .filter(Boolean)
      .join(' ') || undefined
  );
};

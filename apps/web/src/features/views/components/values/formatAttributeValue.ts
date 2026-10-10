import i18n from 'i18next';
import { statusLabel } from '../../../records/status';
import { attributeValueTypes } from '../../../records/valueTypes';
import type { Attribute } from '../../../records/api';
import { formatCalendarDate } from '../../../../time/instantFormat';

/**
 * Plain-text attribute values. Datetime values are instants rendered in the
 * user's zone, so render them with `AttributeValueText` or `<Timestamp>`.
 */

export const formatAttributeValue = (attribute: Attribute, value: unknown) => {
  if (value === null || value === undefined) return i18n.t('views.notSet');
  const label = statusLabel(attribute, value);
  if (label) return label;
  if (attribute.value_type === attributeValueTypes.boolean)
    return value ? i18n.t('views.yes') : i18n.t('views.no');
  if (attribute.value_type === attributeValueTypes.json)
    return JSON.stringify(value);
  if (
    attribute.value_type === attributeValueTypes.date &&
    typeof value === 'string'
  ) {
    return formatCalendarDate(value, i18n.resolvedLanguage ?? i18n.language);
  }
  if (
    attribute.value_type === attributeValueTypes.time &&
    typeof value === 'object' &&
    value !== null
  ) {
    const time = (value as { time?: unknown; time_zone?: unknown }).time;
    const zone = (value as { time_zone?: unknown }).time_zone;
    return typeof time === 'string' && typeof zone === 'string'
      ? `${time} ${zone}`
      : i18n.t('views.invalidTime');
  }
  return String(value);
};

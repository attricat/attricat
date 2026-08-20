import type { Attribute } from '../../../entities/api';

export const formatAttributeValue = (attribute: Attribute, value: unknown) => {
  if (value === null || value === undefined) return 'Not set';
  if (attribute.value_type === 'boolean') return value ? 'Yes' : 'No';
  if (attribute.value_type === 'date' && typeof value === 'string') {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeZone: 'UTC',
    }).format(new Date(`${value}T00:00:00Z`));
  }
  if (attribute.value_type === 'datetime' && typeof value === 'string') {
    return new Intl.DateTimeFormat(undefined, {
      dateStyle: 'medium',
      timeStyle: 'short',
    }).format(new Date(value));
  }
  if (
    attribute.value_type === 'time' &&
    typeof value === 'object' &&
    value !== null
  ) {
    const time = (value as { time?: unknown; time_zone?: unknown }).time;
    const zone = (value as { time_zone?: unknown; time_zone?: unknown })
      .time_zone;
    return typeof time === 'string' && typeof zone === 'string'
      ? `${time} ${zone}`
      : 'Invalid time';
  }
  return String(value);
};

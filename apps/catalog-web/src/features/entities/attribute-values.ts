import type { Attribute, NewAttributeValue } from './api';

type ScalarAttribute = Exclude<Attribute, { value_type: 'relationship' }>;

export const valueForField = (
  value: Extract<NewAttributeValue, { kind: 'scalar' }>['value'] | undefined,
): string => {
  if (typeof value === 'object' && value !== null) {
    return `${value.time} ${value.time_zone}`;
  }
  return value === undefined ? '' : String(value);
};

export const scalarValueForField = (
  attribute: ScalarAttribute,
  fieldValue: string,
): NewAttributeValue | undefined => {
  const value = fieldValue.trim();
  if (!value) return undefined;
  if (attribute.value_type === 'boolean') {
    if (value !== 'true' && value !== 'false') return undefined;
    return {
      kind: 'scalar',
      attribute_code: attribute.code,
      value: value === 'true',
    };
  }
  if (attribute.value_type === 'number') {
    const number = Number(value);
    return Number.isFinite(number)
      ? { kind: 'scalar', attribute_code: attribute.code, value: number }
      : undefined;
  }
  if (attribute.value_type === 'integer') {
    const number = Number(value);
    return Number.isSafeInteger(number)
      ? { kind: 'scalar', attribute_code: attribute.code, value: number }
      : undefined;
  }
  if (attribute.value_type === 'time') {
    const [time, time_zone] = value.split(/\s+/, 2);
    return time && time_zone
      ? {
          kind: 'scalar',
          attribute_code: attribute.code,
          value: { time, time_zone },
        }
      : undefined;
  }
  return { kind: 'scalar', attribute_code: attribute.code, value };
};

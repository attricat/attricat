import { z } from 'zod';
import type { Attribute, NewAttributeValue } from './api';

const timeZoneSchema = z.string().refine(
  (value) => {
    try {
      Intl.DateTimeFormat(undefined, { timeZone: value });
      return true;
    } catch {
      return false;
    }
  },
  'Expected an IANA time zone',
);

const scalarValueSchemas = {
  string: z.string(),
  number: z.coerce.number().finite(),
  integer: z.coerce.number().int().safe(),
  boolean: z.enum(['true', 'false']).transform((value) => value === 'true'),
  date: z.iso.date(),
  datetime: z.iso.datetime({ offset: true }),
  time: z
    .string()
    .transform((value) => value.split(/\s+/, 2))
    .pipe(z.tuple([z.iso.time(), timeZoneSchema]))
    .transform(([time, time_zone]) => ({ time, time_zone })),
};

export const valueForField = (
  value: Extract<NewAttributeValue, { kind: 'scalar' }>['value'] | undefined,
): string => {
  if (typeof value === 'object' && value !== null) {
    return `${value.time} ${value.time_zone}`;
  }
  return value === undefined ? '' : String(value);
};

export const scalarValueForField = (
  attribute: Attribute,
  fieldValue: string,
): NewAttributeValue | undefined => {
  const value = fieldValue.trim();
  if (!value) return undefined;
  if (attribute.value_type === 'relationship') return undefined;
  const schema = scalarValueSchemas[attribute.value_type];
  const result = schema.safeParse(value);
  return result.success
    ? { kind: 'scalar', attribute_code: attribute.code, value: result.data }
    : undefined;
};

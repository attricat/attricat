import { z } from 'zod';
import type { Attribute, NewAttributeValue } from './api';
import { attributeValueKinds, attributeValueTypes } from './value-types';

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
  [attributeValueTypes.string]: z.string(),
  [attributeValueTypes.number]: z.coerce.number().finite(),
  [attributeValueTypes.integer]: z.coerce.number().int().safe(),
  [attributeValueTypes.boolean]: z
    .enum(['true', 'false'])
    .transform((value) => value === 'true'),
  [attributeValueTypes.date]: z.iso.date(),
  [attributeValueTypes.datetime]: z.iso.datetime({ offset: true }),
  [attributeValueTypes.time]: z
    .string()
    .transform((value) => value.split(/\s+/, 2))
    .pipe(z.tuple([z.iso.time(), timeZoneSchema]))
    .transform(([time, time_zone]) => ({ time, time_zone })),
};

export const valueForField = (
  value:
    | Extract<
        NewAttributeValue,
        { kind: typeof attributeValueKinds.scalar }
      >['value']
    | undefined,
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
  if (attribute.value_type === attributeValueTypes.relationship)
    return undefined;
  const schema = scalarValueSchemas[attribute.value_type];
  const result = schema.safeParse(value);
  return result.success
    ? {
        kind: attributeValueKinds.scalar,
        attribute_code: attribute.code,
        value: result.data,
      }
    : undefined;
};

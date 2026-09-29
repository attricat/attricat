import { z } from 'zod';
import i18n from '../../i18n';
import type { Attribute, NewAttributeValue } from './api';
import { validatesJsonSchema } from './jsonSchema';
import { isValidTimeZone } from '../../time/instantFormat';
import { attributeValueKinds, attributeValueTypes } from './valueTypes';
import { booleanFieldValues } from './constants';

const timeZoneSchema = z.string().refine(isValidTimeZone, {
  error: () => i18n.t('entities.invalidTimeZone'),
});

const scalarValueSchemas = {
  [attributeValueTypes.string]: z.string(),
  [attributeValueTypes.number]: z.coerce.number().finite(),
  [attributeValueTypes.integer]: z.coerce.number().int().safe(),
  [attributeValueTypes.boolean]: z
    .enum([booleanFieldValues.true, booleanFieldValues.false])
    .transform((value) => value === booleanFieldValues.true),
  [attributeValueTypes.date]: z.iso.date(),
  [attributeValueTypes.datetime]: z.iso.datetime({ offset: true }),
  [attributeValueTypes.time]: z
    .string()
    .transform((value) => value.split(/\s+/, 2))
    .pipe(z.tuple([z.iso.time(), timeZoneSchema]))
    .transform(([time, time_zone]) => ({ time, time_zone })),
  [attributeValueTypes.json]: z.string().transform((value, context) => {
    try {
      return JSON.parse(value);
    } catch {
      context.addIssue({
        code: 'custom',
        message: i18n.t('entities.invalidJson'),
      });
      return z.NEVER;
    }
  }),
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
    if ('time' in value && 'time_zone' in value) {
      return `${value.time} ${value.time_zone}`;
    }
    return JSON.stringify(value);
  }
  return value === undefined ? '' : String(value);
};

export const scalarValueForField = (
  attribute: Attribute,
  fieldValue: string,
):
  | Extract<NewAttributeValue, { kind: typeof attributeValueKinds.scalar }>
  | undefined => {
  const value = fieldValue.trim();
  if (!value) return undefined;
  if (
    attribute.value_type === attributeValueTypes.relationship ||
    attribute.value_type === attributeValueTypes.file
  )
    return undefined;
  const schema = scalarValueSchemas[attribute.value_type];
  const result = schema.safeParse(value);
  return result.success &&
    validatesJsonSchema(result.data, attribute.value_schema)
    ? {
        kind: attributeValueKinds.scalar,
        attribute_code: attribute.code,
        value: result.data,
      }
    : undefined;
};

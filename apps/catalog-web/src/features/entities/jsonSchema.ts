import Ajv, { type ErrorObject } from 'ajv';
import i18n from '../../i18n';
import { isEmailAddress } from '../views/email';
import type { JsonSchema } from './schemas';

const ajv = new Ajv({ strict: false });
ajv.addFormat('email', isEmailAddress);

export const validatesJsonSchema = (
  value: unknown,
  schema: JsonSchema | null | undefined,
) => {
  return jsonSchemaValidationMessage(value, schema) === undefined;
};

export const jsonSchemaValidationErrors = (
  value: unknown,
  schema: JsonSchema | null | undefined,
): ErrorObject[] | undefined => {
  if (schema == null) return [];

  try {
    if (ajv.validate(schema, value)) return [];
    return ajv.errors ?? [];
  } catch {
    return undefined;
  }
};

export const jsonSchemaValidationMessage = (
  value: unknown,
  schema: JsonSchema | null | undefined,
): string | undefined => {
  const errors = jsonSchemaValidationErrors(value, schema);
  if (errors === undefined) return i18n.t('entities.schemaValidationFailed');
  return errors[0]?.message;
};

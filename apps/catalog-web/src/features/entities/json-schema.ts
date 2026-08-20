import Ajv, { type ErrorObject } from 'ajv';
import type { JsonSchema } from './schemas';

const ajv = new Ajv({ strict: false });

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
  if (errors === undefined) return 'Does not meet the schema requirements.';
  return errors[0]?.message;
};

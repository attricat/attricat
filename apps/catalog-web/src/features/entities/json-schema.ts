import Ajv from 'ajv';
import type { JsonSchema } from './schemas';

const ajv = new Ajv({ strict: false });

export const validatesJsonSchema = (
  value: unknown,
  schema: JsonSchema | null | undefined,
) => {
  return jsonSchemaValidationMessage(value, schema) === undefined;
};

export const jsonSchemaValidationMessage = (
  value: unknown,
  schema: JsonSchema | null | undefined,
): string | undefined => {
  if (schema == null) return undefined;

  try {
    if (ajv.validate(schema, value)) return undefined;
    return ajv.errors?.[0]?.message ?? 'Does not meet the schema requirements.';
  } catch {
    return 'Does not meet the schema requirements.';
  }
};

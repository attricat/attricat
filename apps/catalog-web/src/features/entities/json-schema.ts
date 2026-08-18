import Ajv from 'ajv';
import type { JsonSchema } from './schemas';

const ajv = new Ajv({ strict: false });

export const validatesJsonSchema = (
  value: unknown,
  schema: JsonSchema | null | undefined,
) => {
  if (schema == null) return true;

  try {
    return ajv.validate(schema, value);
  } catch {
    return false;
  }
};

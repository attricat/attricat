import type { z } from 'zod';
import type { Attribute } from './api';
import { attributeValueTypes } from './valueTypes';

/**
 * Reads a configuration a string attribute keeps under a key of its value
 * schema. Every string cell of a table asks for its column's configuration, so
 * each attribute's is parsed once.
 */
export const stringAttributeConfiguration = <T>(
  schemaKey: string,
  schema: z.ZodType<T>,
) => {
  const configurations = new WeakMap<Attribute, T | undefined>();
  return (attribute: Attribute): T | undefined => {
    if (
      attribute.value_type !== attributeValueTypes.string ||
      !attribute.value_schema ||
      typeof attribute.value_schema !== 'object'
    )
      return undefined;
    if (configurations.has(attribute)) return configurations.get(attribute);
    const result = schema.safeParse(attribute.value_schema[schemaKey]);
    const configuration = result.success ? result.data : undefined;
    configurations.set(attribute, configuration);
    return configuration;
  };
};

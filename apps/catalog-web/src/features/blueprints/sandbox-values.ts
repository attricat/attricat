import { scalarValueForField } from '../entities/attribute-values';
import type { Attribute } from '../entities/api';

export type SandboxValues = Record<string, { value: unknown }>;

export const sandboxValuesForFields = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
): SandboxValues =>
  Object.fromEntries(
    attributes.flatMap((attribute) => {
      const value = scalarValueForField(
        attribute,
        fields[attribute.code] ?? '',
      );
      return value ? [[attribute.code, { value: value.value }]] : [];
    }),
  );

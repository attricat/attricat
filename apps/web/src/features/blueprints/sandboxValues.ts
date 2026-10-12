import { scalarValueForField } from '../records/attributeValues';
import type { Attribute, JsonSchema } from '../records/api';
import { validateRecordForm } from '../records/recordForm';

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

export type SandboxValidation = {
  fieldErrors: Record<string, string>;
  formError?: string;
};

/**
 * Checks sandbox input as the record form would, against each attribute's
 * value schema and the record schema. Only fields someone has filled in are
 * reported, so an untouched sandbox shows no errors.
 */
export const sandboxValidation = (
  attributes: readonly Attribute[],
  fields: Record<string, string>,
  recordSchema?: JsonSchema | null,
): SandboxValidation => {
  const touched = Object.keys(fields);
  if (touched.length === 0) return { fieldErrors: {} };
  const { fieldErrors, formError } = validateRecordForm(
    attributes,
    fields,
    [],
    recordSchema,
  );
  return {
    fieldErrors: Object.fromEntries(
      Object.entries(fieldErrors).filter(([code]) => touched.includes(code)),
    ),
    formError,
  };
};

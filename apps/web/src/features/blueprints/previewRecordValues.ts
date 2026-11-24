import { scalarValueForField, valueForField } from '../records/attributeValues';
import type { Attribute } from '../records/api';
import { attributeValueTypes } from '../records/valueTypes';
import { isRelationshipValue } from '../records/linkedRecord';
import type { SandboxValues } from './sandboxValues';

/** Whether the sandbox edits an attribute's value; others come from a record. */
export const isSandboxEditable = (attribute: Attribute) =>
  attribute.value_type !== attributeValueTypes.relationship &&
  attribute.value_type !== attributeValueTypes.file;

/** A chosen record's values, fitted to the previewed attributes. */
export type PreviewRecordValues = {
  /** Sandbox input for the scalar attributes, as the record holds them. */
  fields: Record<string, string>;
  /** Relationship and file values, which the sandbox cannot edit. */
  linked: SandboxValues;
  /** Attributes whose stored value does not fit their previewed type. */
  unfit: string[];
};

/**
 * Fits a record's resolved values, keyed by attribute code, to the
 * attributes of the revision or draft being previewed. A value whose
 * attribute changed type in the draft is left out and reported, so the
 * preview never renders a value as a type it is not.
 */
export const previewRecordValues = (
  attributes: readonly Attribute[],
  values: Readonly<Record<string, { value: unknown }>>,
): PreviewRecordValues => {
  const fields: Record<string, string> = {};
  const linked: SandboxValues = {};
  const unfit: string[] = [];
  for (const attribute of attributes) {
    const stored = values[attribute.code];
    if (stored === undefined || stored.value === null) continue;
    const { value } = stored;
    if (attribute.value_type === attributeValueTypes.relationship) {
      if (isRelationshipValue(value)) linked[attribute.code] = { value };
      else unfit.push(attribute.code);
    } else if (attribute.value_type === attributeValueTypes.file) {
      if (Array.isArray(value)) linked[attribute.code] = { value };
      else unfit.push(attribute.code);
    } else {
      const field = Array.isArray(value)
        ? undefined
        : valueForField(value as Parameters<typeof valueForField>[0]);
      if (field !== undefined && scalarValueForField(attribute, field))
        fields[attribute.code] = field;
      else unfit.push(attribute.code);
    }
  }
  return { fields, linked, unfit };
};

/**
 * The values the preview renders: the sandbox's for what it edits, and the
 * chosen record's relationships and files.
 */
export const mergePreviewValues = (
  attributes: readonly Attribute[],
  sandbox: SandboxValues,
  linked: SandboxValues,
): SandboxValues =>
  Object.fromEntries(
    attributes.flatMap((attribute) => {
      const value = isSandboxEditable(attribute)
        ? sandbox[attribute.code]
        : linked[attribute.code];
      return value ? [[attribute.code, value]] : [];
    }),
  );

import { z } from 'zod';
import { ApiRequestError } from '../../api/request';
import type { Attribute, updateRecord } from './api';
import {
  relationshipTargetsForForm,
  serializeAttributeValues,
  type FieldEditRules,
} from './recordForm';
import { RECORD_SCHEMA_MISMATCH_ERROR_CODE } from './constants';
import { attributeValueTypes } from './valueTypes';

export type FieldSaveRequest = Omit<
  Parameters<typeof updateRecord>[1],
  'expected_updated_at'
>;

/**
 * The partial update that saves `changes` in one context. Unlisted attributes
 * are left untouched by the server; a listed relationship replaces its whole
 * target set and a blank scalar that was saved before is removed.
 */
export const fieldSaveRequest = (
  attributes: readonly Attribute[],
  changes: Readonly<Record<string, string>>,
  saved: Readonly<Record<string, string>>,
  contextId: string | null,
  fieldRules: ReadonlyMap<string, FieldEditRules> = new Map(),
): FieldSaveRequest => {
  // Files save through their own endpoint and are never part of a field save.
  const changed = attributes.filter(
    (attribute) =>
      changes[attribute.code] !== undefined &&
      attribute.value_type !== attributeValueTypes.file,
  );
  return {
    values: serializeAttributeValues(changed, changes, contextId, fieldRules),
    relationships: relationshipTargetsForForm(changed, changes, contextId),
    remove_values: changed
      .filter(
        (attribute) =>
          attribute.value_type !== attributeValueTypes.relationship &&
          !changes[attribute.code]?.trim() &&
          Boolean(saved[attribute.code]?.trim()),
      )
      .map((attribute) => ({
        attribute_code: attribute.code,
        context_id: contextId,
      })),
  };
};

const schemaMismatchDetailsSchema = z.object({
  instance_path: z.string().default(''),
});
const requiredPropertyPattern = /"([^"]+)" is a required property/;

/**
 * The field an `record_schema_mismatch` rejection is about: the first segment
 * of its instance path, or the missing property of a `required` failure.
 */
export const schemaMismatchField = (
  error: unknown,
  fieldCodes: readonly string[],
): { code: string; missing: boolean } | undefined => {
  if (
    !(error instanceof ApiRequestError) ||
    error.code !== RECORD_SCHEMA_MISMATCH_ERROR_CODE
  )
    return undefined;
  const details = schemaMismatchDetailsSchema.safeParse(error.details);
  if (!details.success) return undefined;
  const [segment] = details.data.instance_path.split('/').filter(Boolean);
  const missing = segment
    ? undefined
    : requiredPropertyPattern.exec(error.message)?.[1];
  const code = segment?.replaceAll('~1', '/').replaceAll('~0', '~') ?? missing;
  return code && fieldCodes.includes(code)
    ? { code, missing: missing !== undefined }
    : undefined;
};

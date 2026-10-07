import type { Attribute, updateEntity } from './api';
import {
  relationshipTargetsForForm,
  serializeAttributeValues,
  type FieldEditRules,
} from './entityForm';
import { attributeValueTypes } from './valueTypes';

export type FieldSaveRequest = Omit<
  Parameters<typeof updateEntity>[1],
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

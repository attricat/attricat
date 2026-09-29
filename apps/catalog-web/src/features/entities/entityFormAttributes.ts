import type { Attribute, FormAttributeValue } from './api';
import type { FileMetadata } from '../files/schemas';
import { isHiddenByDefault } from './attributeVisibility';
import {
  attributeContextEditability,
  REUSABLE_ATTRIBUTE_NAMESPACE_SEPARATOR,
} from './constants';
import { attributeValueKinds, attributeValueTypes } from './valueTypes';

export type ResolvedFormValues = Record<
  string,
  { value: unknown; source_context: { id: string; code: string } }
>;

export type RemovedAttributeValue = {
  attribute_code: string;
  context_id: string | null;
};

const inContext = (
  value: { context_id?: string | null },
  contextId: string | null,
) => (value.context_id ?? null) === contextId;

/** Whether an attribute can only be edited in the default context. */
export const isDefaultContextOnly = (
  attribute: Attribute,
  contextId: string | null,
  defaultContextId: string | null,
) =>
  contextId !== defaultContextId &&
  attribute.context_editable === attributeContextEditability.default;

/** Attributes the current user can change in the selected context. */
export const editableFormAttributes = (
  attributes: readonly Attribute[],
  {
    contextId,
    defaultContextId,
    usesDefaultEditView,
  }: {
    contextId: string | null;
    defaultContextId: string | null;
    usesDefaultEditView: boolean;
  },
) =>
  attributes.filter(
    (attribute) =>
      !attribute.readonly &&
      attribute.extension_type?.available !== false &&
      !isDefaultContextOnly(attribute, contextId, defaultContextId) &&
      (!usesDefaultEditView ||
        attribute.code.includes(REUSABLE_ATTRIBUTE_NAMESPACE_SEPARATOR) ||
        !isHiddenByDefault(attribute, 'form')),
  );

/** Local scalar values that were cleared and must be removed on save. */
export const removedFormValues = (
  existingValues: readonly FormAttributeValue[],
  editableAttributes: readonly Attribute[],
  fields: Record<string, string>,
  contextId: string | null,
): RemovedAttributeValue[] =>
  existingValues
    .filter(
      (item) =>
        item.kind === attributeValueKinds.scalar &&
        inContext(item, contextId) &&
        editableAttributes.some(
          (attribute) => attribute.code === item.attribute_code,
        ) &&
        !fields[item.attribute_code]?.trim(),
    )
    .map((item) => ({
      attribute_code: item.attribute_code,
      context_id: contextId,
    }));

/** Keeps only smart fill suggestions for editable scalar attributes. */
export const smartFillFormFields = (
  editableAttributes: readonly Attribute[],
  values: Record<string, string>,
): Record<string, string> => {
  const editableCodes = new Set(
    editableAttributes
      .filter(
        (attribute) =>
          attribute.value_type !== attributeValueTypes.relationship &&
          attribute.value_type !== attributeValueTypes.file,
      )
      .map((attribute) => attribute.code),
  );
  return Object.fromEntries(
    Object.entries(values).filter(([code]) => editableCodes.has(code)),
  );
};

export const hasLocalFormValue = (
  existingValues: readonly FormAttributeValue[],
  attributeCode: string,
  contextId: string | null,
) =>
  existingValues.some(
    (item) =>
      item.attribute_code === attributeCode && inContext(item, contextId),
  );

export const filesForAttribute = (
  existingValues: readonly FormAttributeValue[],
  attributeCode: string,
  contextId: string | null,
): FileMetadata[] =>
  existingValues.find(
    (
      item,
    ): item is Extract<
      FormAttributeValue,
      { kind: typeof attributeValueKinds.file }
    > =>
      item.kind === attributeValueKinds.file &&
      item.attribute_code === attributeCode &&
      inContext(item, contextId),
  )?.files ?? [];

export const formatResolvedValue = (value: unknown) =>
  typeof value === 'object' ? JSON.stringify(value) : String(value);

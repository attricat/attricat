import {
  viewBlockTypes,
  type Attribute,
  type FormAttributeValue,
  type ViewDefinition,
} from './api';
import type { FileMetadata } from '../files/schemas';
import { isHiddenByDefault } from './attributeVisibility';
import {
  attributeContextEditability,
  REUSABLE_ATTRIBUTE_NAMESPACE_SEPARATOR,
} from './constants';
import { attributeValueKinds, attributeValueTypes } from './valueTypes';
import {
  componentPlacedFields,
  viewPlacedFields,
} from '../views/viewFieldComponents';
import { entityHeadingComponentId } from '../views/components/blocks/EntityHeadingDefinition';

export type ResolvedFormValues = Record<
  string,
  { value: unknown; source_context: { id: string; code: string } }
>;

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

/** Attribute codes listed in an entity schema's top-level `required`. */
export const entitySchemaRequiredAttributes = (schema: unknown): string[] =>
  typeof schema === 'object' &&
  schema !== null &&
  'required' in schema &&
  Array.isArray(schema.required)
    ? schema.required.filter((code): code is string => typeof code === 'string')
    : [];

/**
 * Required attributes the view does not render. A form shows them after the
 * view so Save is never blocked by a field the user cannot see.
 */
export const unplacedRequiredAttributes = (
  attributes: readonly Attribute[],
  view: ViewDefinition,
  requiredCodes: readonly string[],
) => {
  // Non-layout views render the default form; see EntityView.
  const usesFallback =
    view.type === viewBlockTypes.table ||
    view.type === viewBlockTypes.dropdownOption ||
    view.type === viewBlockTypes.extensionLayout;
  const placed = viewPlacedFields(view);
  return attributes.filter(
    (attribute) =>
      requiredCodes.includes(attribute.code) &&
      (usesFallback
        ? isHiddenByDefault(attribute, 'form')
        : !placed.has(attribute.code)),
  );
};

/**
 * Editable attributes the detail view does not render, so an editor can
 * still offer them.
 */
export const unplacedEditableAttributes = (
  attributes: readonly Attribute[],
  view: ViewDefinition | undefined,
) => {
  const usesFallback =
    !view ||
    view.type === viewBlockTypes.table ||
    view.type === viewBlockTypes.dropdownOption ||
    view.type === viewBlockTypes.extensionLayout;
  const placed = viewPlacedFields(view);
  return attributes.filter((attribute) =>
    usesFallback
      ? isHiddenByDefault(attribute, 'detail') &&
        !isHiddenByDefault(attribute, 'form')
      : !placed.has(attribute.code) && !isHiddenByDefault(attribute, 'form'),
  );
};

/**
 * Editable attributes the entity heading displays. The heading shows values
 * only, so editors offer these fields first, in heading order.
 */
export const headingEditableAttributes = (
  attributes: readonly Attribute[],
  view: ViewDefinition | undefined,
) =>
  componentPlacedFields(view, entityHeadingComponentId).flatMap((code) =>
    attributes.filter((attribute) => attribute.code === code),
  );

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

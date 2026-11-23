import { principalConfiguration } from '../principals/principal';
import type { Attribute, ComponentReference, RecordFormResponse } from './api';
import { choiceEditorOptions } from './choices';
import { movementInputCodes } from './movements';
import { editableFormAttributes } from './recordFormAttributes';
import { statusConfiguration, statusLocks } from './status';
import { attributeValueTypes } from './valueTypes';

/** Changes of these kinds are final choices rather than typing. */
export const commitsImmediately = (
  attribute: Attribute,
  component: ComponentReference | undefined,
) =>
  attribute.value_type === attributeValueTypes.relationship ||
  attribute.value_type === attributeValueTypes.boolean ||
  attribute.value_type === attributeValueTypes.file ||
  statusConfiguration(attribute) !== undefined ||
  choiceEditorOptions(attribute, component) !== undefined ||
  principalConfiguration(attribute) !== undefined;

/**
 * The attributes the user may change in place: writable here, not locked by
 * a saved status, and not read by a movement that already applied.
 */
export const changeableInlineAttributes = (
  form: RecordFormResponse,
  attributes: readonly Attribute[],
  {
    contextId,
    defaultContextId,
    statusParentContextIds,
  }: {
    contextId: string | null;
    defaultContextId: string | null;
    statusParentContextIds: readonly string[];
  },
) => {
  const existingValues = [...form.values, ...form.reusable_values];
  const editable = form.can_write
    ? editableFormAttributes(attributes, { contextId, defaultContextId })
    : [];
  const locks = statusLocks(
    attributes,
    existingValues,
    contextId,
    statusParentContextIds,
  );
  const movementInputs = movementInputCodes(
    form.blueprint.blueprint.movements,
    form.applied_movements,
  );
  // A movement applies once; what it read stays as it was.
  const changeable = editable.filter(
    (attribute) =>
      locks[attribute.code] === undefined &&
      !movementInputs.has(attribute.code),
  );
  return { changeable, locks, movementInputs };
};

import type { Attribute } from '../../records/api';
import { attributeValueTypes } from '../../records/valueTypes';
import type { ExtensionContribution } from '../api';
import {
  contributionKinds,
  extensionOutlets,
  renderingModes,
} from '../constants';

/** How a detail field names an extension's attribute editor. */
export type EditorReference = {
  id: string;
  version: number;
  props: Record<string, unknown>;
};

const numeric = new Set<string>([
  attributeValueTypes.number,
  attributeValueTypes.integer,
]);

/**
 * The enabled, granted native editor a field names, when it edits the
 * field's value type. Anything else leaves the field its default control.
 */
export const resolveAttributeEditor = (
  runtime: readonly ExtensionContribution[] | undefined,
  reference: EditorReference | null | undefined,
  attribute: Attribute,
) =>
  reference
    ? runtime?.find(
        (contribution) =>
          contribution.kind === contributionKinds.editor &&
          contribution.outlet === extensionOutlets.attributeEditor &&
          contribution.rendering === renderingModes.native &&
          contribution.id === reference.id &&
          contribution.version === reference.version &&
          contribution.editor?.value_types.includes(attribute.value_type),
      )
    : undefined;

/** The inline form's text of a field, as the value an editor sees. */
export const editorValue = (attribute: Attribute, text: string) => {
  if (text === '') return null;
  if (numeric.has(attribute.value_type)) {
    const parsed = Number(text);
    return Number.isFinite(parsed) ? parsed : null;
  }
  if (attribute.value_type === attributeValueTypes.boolean)
    return text === 'true';
  return text;
};

/**
 * The inline form's text for a value an editor proposes, or undefined when
 * the value cannot be one of the field's type. The host still validates the
 * text against the attribute before saving it.
 */
export const proposedText = (attribute: Attribute, value: unknown) => {
  if (value === null) return '';
  if (numeric.has(attribute.value_type))
    return typeof value === 'number' && Number.isFinite(value)
      ? String(value)
      : undefined;
  if (attribute.value_type === attributeValueTypes.boolean)
    return typeof value === 'boolean' ? String(value) : undefined;
  return typeof value === 'string' ? value : undefined;
};

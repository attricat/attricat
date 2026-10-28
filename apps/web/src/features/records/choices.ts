import { principalConfiguration } from '../principals/principal';
import { resolveValueEditor } from '../views/components/registry';
import type { Attribute, ComponentReference } from './api';
import { STATUS_SCHEMA_KEY } from './status';
import { attributeValueTypes } from './valueTypes';
import { enumValueLabel } from './enumLabels';

export type ChoiceOption = { code: string; label: string };

/**
 * The fixed values of a string attribute whose `value_schema` has an `enum`,
 * labelled by `x-attricat-enum-labels` where given and by the value
 * otherwise. Statuses and user-or-team attributes have their own controls.
 */
export const choiceOptions = (
  attribute: Attribute,
): ChoiceOption[] | undefined => {
  const schema = attribute.value_schema;
  if (
    attribute.value_type !== attributeValueTypes.string ||
    !schema ||
    typeof schema !== 'object' ||
    STATUS_SCHEMA_KEY in schema ||
    principalConfiguration(attribute)
  )
    return undefined;
  const values = schema.enum;
  if (
    !Array.isArray(values) ||
    values.length === 0 ||
    !values.every((value): value is string => typeof value === 'string')
  )
    return undefined;
  return values.map((code) => ({
    code,
    label: enumValueLabel(schema, code) ?? code,
  }));
};

/** The configured label of an `enum` attribute's value, if it has one. */
export const choiceValueLabel = (
  attribute: Attribute,
  value: unknown,
): string | undefined => {
  const options = choiceOptions(attribute);
  return options?.find((option) => option.code === value)?.label;
};

/**
 * The choices to offer in a select, unless the view configures its own edit
 * component for the attribute, which then takes precedence over the `enum`.
 */
export const choiceEditorOptions = (
  attribute: Attribute,
  component: ComponentReference | null | undefined,
): ChoiceOption[] | undefined =>
  resolveValueEditor(component, attribute)
    ? undefined
    : choiceOptions(attribute);

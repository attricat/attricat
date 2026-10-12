import { lexiconText } from '../lexicon/lexicon';
import { principalConfiguration } from '../principals/principal';
import { resolveValueEditor } from '../views/components/registry';
import type { Attribute, ComponentReference } from './api';
import { STATUS_SCHEMA_KEY } from './status';

export const ENUM_LABELS_SCHEMA_KEY = 'x-attricat-enum-labels';

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
    attribute.value_type !== 'string' ||
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
  const labels = schema[ENUM_LABELS_SCHEMA_KEY];
  const labelFor = (code: string) => {
    const label =
      labels && typeof labels === 'object' && !Array.isArray(labels)
        ? (labels as Record<string, unknown>)[code]
        : undefined;
    return typeof label === 'string' && label.trim()
      ? lexiconText(label)
      : code;
  };
  return values.map((code) => ({ code, label: labelFor(code) }));
};

/** The configured label of an `enum` attribute's value, if it has one. */
export const choiceValueLabel = (
  attribute: Attribute,
  value: unknown,
): string | undefined => {
  const options = choiceOptions(attribute);
  return options?.find((option) => option.code === value)?.label;
};

/** The label of a configured value, or the value itself. */
export const choiceLabel = (
  options: readonly ChoiceOption[],
  value: unknown,
): string =>
  options.find((option) => option.code === value)?.label ?? String(value);

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

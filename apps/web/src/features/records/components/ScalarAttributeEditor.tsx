import { Box, TextField } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type {
  Attribute,
  ComponentReference,
  StatusTransitionAccess,
} from '../api';
import { resolveValueEditor } from '../../views/components/registry';
import type { ValueEditorProps } from '../../views/components/componentTypes';
import { StatusEditor } from '../../views/controls/editors';
import {
  BooleanEditor,
  ChoiceEditor,
} from '../../views/controls/choiceControls';
import { choiceEditorOptions } from '../choices';
import { attributeLabel } from '../recordDisplay';
import { attributeValueTypes } from '../valueTypes';
import { JSON_EDITOR_MIN_ROWS, scalarValuePlaceholders } from '../constants';
import { statusConfiguration } from '../status';
import { principalConfiguration } from '../../principals/principal';
import { PrincipalEditor } from '../../principals/PrincipalEditor';
import { attributeMaxLength } from '../attributeHints';
import { FieldHint } from '../../../components/FieldHint';

const isJson = (text: string) => {
  try {
    JSON.parse(text);
    return true;
  } catch {
    return false;
  }
};

const numeric = (attribute: Attribute) =>
  attribute.value_type === attributeValueTypes.number ||
  attribute.value_type === attributeValueTypes.integer;

/** Helper text with a count of the characters used against the limit. */
const CountedHelperText = ({
  text,
  length,
  max,
}: {
  text?: ReactNode;
  length: number;
  max: number;
}) => (
  <Box
    component="span"
    sx={{ display: 'flex', gap: 2, justifyContent: 'space-between' }}
  >
    <span>{text}</span>
    <Box
      component="span"
      sx={{ flexShrink: 0, color: length > max ? 'error.main' : undefined }}
    >
      {`${length} / ${max}`}
    </Box>
  </Box>
);

/** The built-in input for a scalar value type. */
const BuiltInEditor = (props: ValueEditorProps) =>
  props.attribute.value_type === attributeValueTypes.boolean ? (
    <BooleanEditor {...props} />
  ) : (
    <TextValueEditor {...props} />
  );

/** A typed text input, for every scalar type but yes or no. */
const TextValueEditor = ({
  attribute,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
}: ValueEditorProps) => {
  const { t } = useTranslation();
  const json = attribute.value_type === attributeValueTypes.json;
  const date = attribute.value_type === attributeValueTypes.date;
  const maxLength = attributeMaxLength(attribute);
  // JSON is checked as it is typed; saving still reports invalid JSON.
  const jsonNote =
    json && value.trim()
      ? isJson(value)
        ? t('records.jsonValid')
        : t('records.jsonNotValidYet')
      : undefined;
  const hint = [helperText, jsonNote].filter(Boolean).join(' ');
  const text = error ?? (hint && <FieldHint>{hint}</FieldHint>);
  return (
    <TextField
      fullWidth
      disabled={disabled}
      required={required}
      error={Boolean(error)}
      helperText={
        maxLength === undefined ? (
          text
        ) : (
          // JSON Schema counts code points, not UTF-16 units.
          <CountedHelperText
            length={[...value].length}
            max={maxLength}
            text={text}
          />
        )
      }
      label={attributeLabel(attribute)}
      value={value}
      onChange={(event) => onChange(event.target.value)}
      multiline={json}
      minRows={json ? JSON_EDITOR_MIN_ROWS : undefined}
      placeholder={scalarValuePlaceholders[attribute.value_type]}
      slotProps={{
        // Native date inputs show date segments even while empty.
        inputLabel: date ? { shrink: true } : undefined,
        htmlInput: { inputMode: numeric(attribute) ? 'decimal' : undefined },
      }}
      type={date ? 'date' : numeric(attribute) ? 'number' : undefined}
    />
  );
};

/**
 * Editor for one scalar attribute value, shared by the record form and the
 * blueprint preview sandbox. A status or user-or-team annotation takes
 * precedence, then the view's configured edit component, then a select for an
 * `enum` of choices, then the built-in input for the type.
 */
export const ScalarAttributeEditor = ({
  component,
  statusBaseline = null,
  inheritedStatus = null,
  statusTransitions,
  ...props
}: ValueEditorProps & {
  component?: ComponentReference | null;
  /** Saved status that transitions start from; `null` for a new value. */
  statusBaseline?: string | null;
  inheritedStatus?: string | null;
  /** The caller's access to each declared edge, when the server provided it. */
  statusTransitions?: readonly StatusTransitionAccess[];
}) => {
  const status = statusConfiguration(props.attribute);
  if (status)
    return (
      <StatusEditor
        {...props}
        config={status}
        baseline={statusBaseline}
        inheritedValue={inheritedStatus}
        transitions={statusTransitions}
      />
    );
  const principal = principalConfiguration(props.attribute);
  if (principal) return <PrincipalEditor {...props} config={principal} />;
  const configured = resolveValueEditor(component, props.attribute);
  if (configured?.valueEditor) return <configured.valueEditor {...props} />;
  const choices = choiceEditorOptions(props.attribute, component);
  if (choices) return <ChoiceEditor {...props} options={choices} />;
  return <BuiltInEditor {...props} />;
};

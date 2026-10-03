import { MenuItem, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { Attribute, ComponentReference } from '../api';
import { resolveValueEditor } from '../../views/components/registry';
import type { ValueEditorProps } from '../../views/components/componentTypes';
import { StatusEditor } from '../../views/controls/editors';
import { attributeLabel } from '../entityDisplay';
import { attributeValueTypes } from '../valueTypes';
import {
  booleanFieldValues,
  JSON_EDITOR_MIN_ROWS,
  scalarValuePlaceholders,
} from '../constants';
import { statusConfiguration } from '../status';
import type { StatusTransitionAccess } from '../recordControls';

const numeric = (attribute: Attribute) =>
  attribute.value_type === attributeValueTypes.number ||
  attribute.value_type === attributeValueTypes.integer;

/** The built-in input for a scalar value type. */
const BuiltInEditor = ({
  attribute,
  value,
  disabled,
  error,
  helperText,
  onChange,
}: ValueEditorProps) => {
  const { t } = useTranslation();
  const common = {
    fullWidth: true,
    disabled,
    error: Boolean(error),
    helperText: error ?? helperText,
    label: attributeLabel(attribute),
    value,
    onChange: (event: { target: { value: string } }) =>
      onChange(event.target.value),
  };
  if (attribute.value_type === attributeValueTypes.boolean)
    return (
      <TextField {...common} select>
        <MenuItem value="">{t('entities.notSet')}</MenuItem>
        <MenuItem value={booleanFieldValues.true}>
          {t('entities.true')}
        </MenuItem>
        <MenuItem value={booleanFieldValues.false}>
          {t('entities.false')}
        </MenuItem>
      </TextField>
    );
  const json = attribute.value_type === attributeValueTypes.json;
  return (
    <TextField
      {...common}
      multiline={json}
      minRows={json ? JSON_EDITOR_MIN_ROWS : undefined}
      placeholder={scalarValuePlaceholders[attribute.value_type]}
      slotProps={{
        htmlInput: { inputMode: numeric(attribute) ? 'decimal' : undefined },
      }}
      type={
        attribute.value_type === attributeValueTypes.date
          ? 'date'
          : numeric(attribute)
            ? 'number'
            : undefined
      }
    />
  );
};

/**
 * Editor for one scalar attribute value, shared by the entity form and the
 * blueprint preview sandbox. A status annotation takes precedence, then the
 * view's configured edit component, then the built-in input for the type.
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
  const Editor =
    resolveValueEditor(component, props.attribute)?.valueEditor ??
    BuiltInEditor;
  return <Editor {...props} />;
};

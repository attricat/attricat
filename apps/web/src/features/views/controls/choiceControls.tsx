import {
  Autocomplete,
  FormControl,
  FormControlLabel,
  FormHelperText,
  FormLabel,
  Radio,
  RadioGroup,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
} from '@mui/material';
import { useId, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { ChoiceOption } from '../../records/choices';
import { booleanFieldValues } from '../../records/constants';
import { attributeLabel } from '../../records/recordDisplay';
import type { ValueEditorProps } from '../components/componentTypes';
import { FieldHint } from '../../../components/FieldHint';

/** Choices at most this many are radio buttons; longer lists are searched. */
const RADIO_CHOICE_LIMIT = 5;

/**
 * Names a group of choice controls the way a text field names its input, with
 * the same required mark, error colour and helper text.
 */
const ChoiceFieldFrame = ({
  label,
  disabled,
  required,
  error,
  helperText,
  children,
}: {
  label: string;
  disabled?: boolean;
  required?: boolean;
  error?: string;
  helperText?: string;
  children: (labelId: string) => ReactNode;
}) => {
  const labelId = useId();
  const helpId = useId();
  const help = error ?? (helperText && <FieldHint>{helperText}</FieldHint>);
  return (
    <FormControl
      aria-describedby={help ? helpId : undefined}
      component="fieldset"
      disabled={disabled}
      error={Boolean(error)}
      fullWidth
      required={required}
    >
      <FormLabel component="legend" id={labelId} sx={{ mb: 0.5, p: 0 }}>
        {label}
      </FormLabel>
      {children(labelId)}
      {help && <FormHelperText id={helpId}>{help}</FormHelperText>}
    </FormControl>
  );
};

/** Not set, yes or no, all visible at once. */
export const BooleanEditor = ({
  attribute,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
}: ValueEditorProps) => {
  const { t } = useTranslation();
  return (
    <ChoiceFieldFrame
      disabled={disabled}
      error={error}
      helperText={helperText}
      label={attributeLabel(attribute)}
      required={required}
    >
      {(labelId) => (
        <ToggleButtonGroup
          aria-labelledby={labelId}
          disabled={disabled}
          exclusive
          onChange={(_, next: string | null) => {
            // Choosing the selected button again keeps it.
            if (!disabled && next !== null) onChange(next);
          }}
          size="small"
          value={value}
        >
          <ToggleButton value="">{t('records.notSet')}</ToggleButton>
          <ToggleButton value={booleanFieldValues.true}>
            {t('views.yes')}
          </ToggleButton>
          <ToggleButton value={booleanFieldValues.false}>
            {t('views.no')}
          </ToggleButton>
        </ToggleButtonGroup>
      )}
    </ChoiceFieldFrame>
  );
};

/**
 * An `enum` attribute's values, labelled where configured: radio buttons for a
 * short list, a searchable list for a long one. A stored value that is no
 * longer allowed stays visible as an error.
 */
export const ChoiceEditor = ({
  attribute,
  options,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
}: ValueEditorProps & { options: readonly ChoiceOption[] }) => {
  const { t } = useTranslation();
  const known = !value || options.some((option) => option.code === value);
  const problem =
    error ?? (known ? undefined : t('records.choiceUnknown', { value }));
  const change = (next: string) => {
    if (!disabled) onChange(next);
  };
  if (options.length > RADIO_CHOICE_LIMIT)
    return (
      <Autocomplete
        disabled={disabled}
        disableClearable={required}
        getOptionLabel={(code) =>
          options.find((option) => option.code === code)?.label ?? code
        }
        // A stored value that is no longer allowed is shown, not offered.
        getOptionDisabled={(code) => !known && code === value}
        onChange={(event, next: string | null, reason) => {
          // Emptying the text to search again is not a choice; on the record
          // page every choice saves at once. The clear button still clears.
          if (reason === 'clear' && event.type === 'change') return;
          change(next ?? '');
        }}
        options={[
          ...options.map((option) => option.code),
          ...(known ? [] : [value]),
        ]}
        renderInput={(params) => (
          <TextField
            {...params}
            error={Boolean(problem)}
            helperText={
              problem ?? (helperText && <FieldHint>{helperText}</FieldHint>)
            }
            label={attributeLabel(attribute)}
            required={required}
          />
        )}
        value={value || null}
      />
    );
  return (
    <ChoiceFieldFrame
      disabled={disabled}
      error={problem}
      helperText={helperText}
      label={attributeLabel(attribute)}
      required={required}
    >
      {(labelId) => (
        <RadioGroup
          aria-labelledby={labelId}
          onChange={(event) => change(event.target.value)}
          row
          value={value}
        >
          {!required && (
            <FormControlLabel
              control={<Radio />}
              label={t('records.notSet')}
              value=""
            />
          )}
          {options.map((option) => (
            <FormControlLabel
              control={<Radio />}
              key={option.code}
              label={option.label}
              value={option.code}
            />
          ))}
        </RadioGroup>
      )}
    </ChoiceFieldFrame>
  );
};

import { Stack, TextField } from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../entityDisplay';
import { COLOR_PICKER_SEED, parseColor } from '../../views/colorValue';
import type { ValueEditorProps } from '../../views/components/componentTypes';

export const ColorAttributeEditor = ({
  attribute,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
}: ValueEditorProps) => {
  const { t } = useTranslation();
  const id = useId();
  const invalid = value !== '' && !parseColor(value);
  const message =
    error ??
    (invalid
      ? t('views.colorInvalid')
      : (helperText ?? t('views.colorFormat')));
  const change = (next: string) => {
    if (!disabled) onChange(next);
  };
  return (
    <Stack direction="row" spacing={1} sx={{ alignItems: 'flex-start' }}>
      <TextField
        id={id}
        fullWidth
        required={required}
        label={attributeLabel(attribute)}
        value={value}
        disabled={disabled}
        error={Boolean(error || invalid)}
        helperText={message}
        onChange={(event) => change(event.target.value)}
        slotProps={{ htmlInput: { spellCheck: false, autoCapitalize: 'none' } }}
      />
      <TextField
        type="color"
        label={t('views.colorPicker')}
        value={parseColor(value) ?? COLOR_PICKER_SEED}
        disabled={disabled}
        onChange={(event) => change(event.target.value)}
        sx={{ minWidth: 100 }}
        slotProps={{
          inputLabel: { shrink: true },
          htmlInput: {
            'aria-label': t('views.colorPickerFor', {
              attribute: attributeLabel(attribute),
            }),
            'aria-describedby': `${id}-helper-text`,
          },
        }}
      />
    </Stack>
  );
};

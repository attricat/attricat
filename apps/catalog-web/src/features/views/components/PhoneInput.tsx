import { TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../../entities/entityDisplay';
import type { ValueEditorProps } from './componentTypes';

export const PhoneAttributeEditor = ({
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
    <TextField
      fullWidth
      type="tel"
      label={attributeLabel(attribute)}
      value={value}
      disabled={disabled}
      required={required}
      error={Boolean(error)}
      helperText={error ?? helperText ?? t('entities.phoneHelp')}
      onChange={(event) => {
        if (!disabled) onChange(event.target.value);
      }}
      slotProps={{ htmlInput: { dir: 'ltr', inputMode: 'tel' } }}
    />
  );
};

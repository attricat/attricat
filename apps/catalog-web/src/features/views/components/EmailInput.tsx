import { TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../../entities/entityDisplay';
import { isEmailAddress } from '../email';
import type { ValueEditorProps } from './componentTypes';

export const EmailInput = ({
  label,
  value,
  onChange,
  disabled = false,
  required = false,
  error,
  helperText,
}: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  required?: boolean;
  error?: string;
  helperText?: string;
}) => {
  const { t } = useTranslation();
  const message =
    error ??
    (value.trim() && !isEmailAddress(value.trim())
      ? t('entities.invalidEmail')
      : undefined);
  return (
    <TextField
      fullWidth
      label={label}
      type="email"
      value={value}
      disabled={disabled}
      required={required}
      error={Boolean(message)}
      helperText={message ?? helperText}
      onChange={(event) => {
        if (!disabled) onChange(event.target.value);
      }}
      slotProps={{
        htmlInput: {
          inputMode: 'email',
          autoCapitalize: 'none',
          spellCheck: false,
        },
      }}
    />
  );
};

export const EmailAttributeEditor = ({
  attribute,
  ...props
}: ValueEditorProps) => (
  <EmailInput label={attributeLabel(attribute)} {...props} />
);

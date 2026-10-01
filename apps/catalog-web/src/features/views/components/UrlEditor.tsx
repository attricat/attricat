import { TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../../entities/entityDisplay';
import { safeUrl } from '../urlPolicy';
import type { ValueEditorProps } from './componentTypes';

export const UrlEditor = ({
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
  const validationError =
    error ?? (value && !safeUrl(value) ? t('views.invalidUrl') : undefined);
  return (
    <TextField
      fullWidth
      label={label}
      type="url"
      value={value}
      disabled={disabled}
      required={required}
      error={Boolean(validationError)}
      helperText={validationError ?? helperText}
      onChange={(event) => {
        if (!disabled) onChange(event.target.value);
      }}
      slotProps={{
        htmlInput: {
          inputMode: 'url',
          autoCapitalize: 'none',
          spellCheck: false,
        },
      }}
    />
  );
};

export const UrlAttributeEditor = ({
  attribute,
  ...props
}: ValueEditorProps) => (
  <UrlEditor label={attributeLabel(attribute)} {...props} />
);

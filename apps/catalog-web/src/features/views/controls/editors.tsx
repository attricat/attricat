import { Stack, TextField } from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../../entities/entityDisplay';
import { MarkdownEditor } from '../../markdown/MarkdownEditor';
import type { ValueEditorProps } from '../components/componentTypes';
import { COLOR_PICKER_SEED, parseColor, validateColor } from './color';
import { validateEmail } from './email';
import { TextControlEditor } from './TextControl';
import { validateUrl } from './url';

export const ColorEditor = ({
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
  const message =
    error ?? validateColor(value) ?? helperText ?? t('views.colorFormat');
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
        error={Boolean(error ?? validateColor(value))}
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

export const EmailEditor = (props: ValueEditorProps) => (
  <TextControlEditor {...props} type="email" validate={validateEmail} />
);

export const UrlEditor = (props: ValueEditorProps) => (
  <TextControlEditor {...props} type="url" validate={validateUrl} />
);

export const PhoneEditor = (props: ValueEditorProps) => {
  const { t } = useTranslation();
  return (
    <TextControlEditor
      {...props}
      type="tel"
      leftToRight
      defaultHelperText={t('entities.phoneHelp')}
    />
  );
};

export const MarkdownFieldEditor = ({
  attribute,
  value,
  disabled,
  error,
  helperText,
  onChange,
}: ValueEditorProps) => (
  <MarkdownEditor
    label={attributeLabel(attribute)}
    value={value}
    disabled={disabled}
    error={error}
    helperText={helperText}
    onChange={onChange}
  />
);

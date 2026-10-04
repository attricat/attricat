import { ListItemText, MenuItem, Stack, TextField } from '@mui/material';
import { useId } from 'react';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../../entities/entityDisplay';
import {
  statusCodeLabel,
  statusOptionLabel,
  statusTransitionDenial,
  type StatusConfiguration,
} from '../../entities/status';
import { useStatusTransitionDenialText } from '../../entities/useStatusTransitionDenialText';
import { MarkdownEditor } from '../../markdown/MarkdownEditor';
import type { ValueEditorProps } from '../components/componentTypes';
import type { StatusTransitionAccess } from '../../entities/api';
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
  const problem = error ?? validateColor(value);
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
        error={Boolean(problem)}
        helperText={problem ?? helperText ?? t('views.colorFormat')}
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
      defaultHelperText={t('views.phoneHelp')}
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

/**
 * Single select for a status attribute. Destinations the transition graph
 * forbids from the saved `baseline` are disabled; an empty selection inherits
 * `inheritedValue` (or clears the status when nothing is inherited). For a
 * saved entity, `destinations` from the API also disables destinations whose
 * transition conditions are unmet and explains why.
 */
export const StatusEditor = ({
  attribute,
  config,
  value,
  baseline,
  inheritedValue,
  transitions = [],
  disabled,
  error,
  helperText,
  onChange,
}: ValueEditorProps & {
  config: StatusConfiguration;
  baseline: string | null;
  inheritedValue: string | null;
  /** Server-evaluated access to edges leaving the saved status. */
  transitions?: readonly StatusTransitionAccess[];
}) => {
  const { t } = useTranslation();
  const id = useId();
  const known =
    !value || config.options.some((option) => option.code === value);
  // Permission, role and separation-of-duties checks need the server; the
  // graph alone cannot say who may take an edge.
  const denialText = useStatusTransitionDenialText();
  const denial = (next: string) =>
    statusTransitionDenial(config, {
      attributeCode: attribute.code,
      baseline,
      inherited: inheritedValue,
      selected: next,
      transitions,
    });
  const allowed = (next: string) => !denial(next);
  const terminal =
    !config.options.some(
      (option) => option.code !== baseline && allowed(option.code),
    ) && !(inheritedValue !== baseline && allowed(''));
  return (
    <TextField
      id={id}
      fullWidth
      select
      label={attributeLabel(attribute)}
      value={value}
      disabled={disabled}
      slotProps={{ select: { readOnly: disabled } }}
      error={Boolean(error) || !known}
      helperText={
        error ??
        (!known
          ? t('entities.statusUnknown', { value })
          : [
              helperText,
              terminal
                ? t('entities.statusTerminal')
                : t('entities.statusTransitions'),
            ]
              .filter(Boolean)
              .join(' '))
      }
      onChange={(event) => {
        if (!disabled && allowed(event.target.value))
          onChange(event.target.value);
      }}
    >
      <MenuItem value="" disabled={!allowed('')}>
        {inheritedValue
          ? t('entities.statusInherit', {
              value: statusCodeLabel(config, inheritedValue) ?? inheritedValue,
            })
          : t('entities.notSet')}
      </MenuItem>
      {!known && (
        <MenuItem value={value} disabled>
          {value}
        </MenuItem>
      )}
      {config.options.map((option) => {
        const denied = denial(option.code);
        return (
          <MenuItem
            key={option.code}
            value={option.code}
            disabled={Boolean(denied)}
          >
            {denied?.kind === 'access' ? (
              <ListItemText
                primary={statusOptionLabel(option)}
                secondary={denialText(denied)}
              />
            ) : (
              statusOptionLabel(option)
            )}
          </MenuItem>
        );
      })}
    </TextField>
  );
};

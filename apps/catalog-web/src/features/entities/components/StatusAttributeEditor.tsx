import { MenuItem, TextField } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { useId } from 'react';
import type { StatusConfiguration } from '../status';
import { statusTransitionAllowed } from '../status';

export const StatusAttributeEditor = ({
  config,
  label,
  value,
  baseline,
  inheritedValue,
  disabled,
  error,
  helperText,
  onChange,
}: {
  config: StatusConfiguration;
  label: string;
  value: string;
  baseline: string | null;
  inheritedValue: string | null;
  disabled: boolean;
  error?: string;
  helperText?: string;
  onChange: (value: string) => void;
}) => {
  const { t } = useTranslation();
  const id = useId();
  const known =
    !value || config.options.some((option) => option.code === value);
  const allowed = (next: string) =>
    statusTransitionAllowed(config, baseline, next || inheritedValue);
  const terminal =
    !config.options.some(
      (option) => option.code !== baseline && allowed(option.code),
    ) && !(inheritedValue !== baseline && allowed(''));
  return (
    <TextField
      id={id}
      fullWidth
      select
      label={label}
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
              value:
                config.options.find((option) => option.code === inheritedValue)
                  ?.label ?? inheritedValue,
            })
          : t('entities.notSet')}
      </MenuItem>
      {!known && (
        <MenuItem value={value} disabled>
          {value}
        </MenuItem>
      )}
      {config.options.map((option) => (
        <MenuItem
          key={option.code}
          value={option.code}
          disabled={!allowed(option.code)}
        >
          {option.label}
        </MenuItem>
      ))}
    </TextField>
  );
};

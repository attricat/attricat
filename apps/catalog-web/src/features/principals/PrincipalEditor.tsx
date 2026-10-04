import { Autocomplete, Stack, TextField, Typography } from '@mui/material';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { AssignedUserIcon, TeamIcon } from '../../components/systemIcons';
import { attributeLabel } from '../entities/entityDisplay';
import type { ValueEditorProps } from '../views/components/componentTypes';
import { assignableOptions, isAssignable, resolvePrincipal } from './principal';
import type { PrincipalConfiguration } from './schemas';
import { usePrincipalDirectory } from './usePrincipalDirectory';

/**
 * Picks a workspace user or team. A saved assignee who can no longer be
 * assigned stays selected until changed, so unrelated edits keep it.
 */
export const PrincipalEditor = ({
  attribute,
  config,
  value,
  disabled,
  required,
  error,
  helperText,
  onChange,
}: ValueEditorProps & { config: PrincipalConfiguration }) => {
  const { t } = useTranslation();
  const directory = usePrincipalDirectory();
  const options = useMemo(() => {
    const assignable = assignableOptions(directory.data, config);
    const current = resolvePrincipal(directory.data, value);
    return current && !assignable.some((option) => option.value === value)
      ? [{ value, principal: current }, ...assignable]
      : assignable;
  }, [config, directory.data, value]);
  const selected = options.find((option) => option.value === value) ?? null;
  const unknown = Boolean(value) && !selected && !directory.isPending;
  return (
    <Autocomplete
      disabled={disabled}
      readOnly={disabled}
      loading={directory.isPending}
      options={options}
      value={selected}
      groupBy={
        config.kinds.length > 1
          ? (option) =>
              t(
                option.principal.kind === 'team'
                  ? 'principals.teams'
                  : 'principals.users',
              )
          : undefined
      }
      getOptionLabel={(option) => option.principal.label}
      getOptionDisabled={(option) => !isAssignable(option.principal)}
      isOptionEqualToValue={(option, current) => option.value === current.value}
      filterOptions={(items, state) => {
        const query = state.inputValue.trim().toLowerCase();
        return items.filter(
          (option) =>
            !query ||
            option.principal.label.toLowerCase().includes(query) ||
            (option.principal.kind === 'user' &&
              option.principal.user.email.toLowerCase().includes(query)) ||
            (option.principal.kind === 'team' &&
              option.principal.team.code.toLowerCase().includes(query)),
        );
      }}
      onChange={(_, option) => {
        if (!disabled) onChange(option?.value ?? '');
      }}
      renderOption={({ key, ...props }, option) => {
        const Icon =
          option.principal.kind === 'team' ? TeamIcon : AssignedUserIcon;
        return (
          <li key={key} {...props}>
            <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
              <Icon aria-hidden size={compactIconSize} />
              <span>{option.principal.label}</span>
              {option.principal.kind === 'user' &&
                option.principal.user.display_name && (
                  <Typography color="text.secondary" variant="caption">
                    {option.principal.user.email}
                  </Typography>
                )}
            </Stack>
          </li>
        );
      }}
      renderInput={(params) => (
        <TextField
          {...params}
          label={attributeLabel(attribute)}
          required={required}
          error={Boolean(error) || unknown}
          helperText={
            error ?? (unknown ? t('principals.unknown', { value }) : helperText)
          }
        />
      )}
    />
  );
};

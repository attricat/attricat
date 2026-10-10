import { Autocomplete, Stack, TextField, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { attributeLabel } from '../records/recordDisplay';
import type { ValueEditorProps } from '../views/components/componentTypes';
import { assignableOptions, isAssignable, resolvePrincipal } from './principal';
import { PrincipalKindIcon } from './PrincipalValue';
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
  const assignable = assignableOptions(directory.data, config);
  const current = resolvePrincipal(directory.data, value);
  const options =
    current && !assignable.some((option) => option.value === value)
      ? [{ value, principal: current }, ...assignable]
      : assignable;
  const selected = options.find((option) => option.value === value) ?? null;
  // A reference is unknown only once the directory has loaded without it. A
  // failed background refetch keeps the loaded directory usable.
  const loadFailed = directory.isLoadingError;
  const unknown = Boolean(value) && !selected && directory.data !== undefined;
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
      renderOption={({ key, ...props }, option) => (
        <li key={key} {...props}>
          <Stack direction="row" spacing={1} sx={{ alignItems: 'center' }}>
            <PrincipalKindIcon kind={option.principal.kind} />
            <span>{option.principal.label}</span>
            {option.principal.kind === 'user' &&
              option.principal.user.display_name && (
                <Typography color="text.secondary" variant="caption">
                  {option.principal.user.email}
                </Typography>
              )}
          </Stack>
        </li>
      )}
      renderInput={(params) => (
        <TextField
          {...params}
          label={attributeLabel(attribute)}
          required={required}
          error={Boolean(error) || loadFailed || unknown}
          helperText={
            error ??
            (loadFailed
              ? t('principals.loadFailed')
              : unknown
                ? t('principals.unknown', { value })
                : helperText)
          }
        />
      )}
    />
  );
};

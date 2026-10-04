import { Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { AssignedUserIcon, TeamIcon } from '../../components/systemIcons';
import { NotSetValue } from '../views/components/values/NotSetValue';
import { isAssignable, resolvePrincipal } from './principal';
import { usePrincipalDirectory } from './usePrincipalDirectory';

/**
 * An assigned user or team as a labelled chip. References the directory no
 * longer lists, or people who left, stay visible with an explanation.
 */
export const PrincipalValue = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  const directory = usePrincipalDirectory(
    value !== null && value !== undefined,
  );
  if (value === null || value === undefined || value === '')
    return <NotSetValue />;
  const principal = resolvePrincipal(directory.data, value);
  if (!principal) {
    return (
      <Typography color="text.secondary" variant="body2">
        {directory.isPending
          ? t('principals.loading')
          : t('principals.unknown', { value: String(value) })}
      </Typography>
    );
  }
  const Icon = principal.kind === 'team' ? TeamIcon : AssignedUserIcon;
  return (
    <Stack spacing={0.5} sx={{ alignItems: 'flex-start' }}>
      <Chip
        icon={<Icon aria-hidden size={compactIconSize} />}
        label={principal.label}
        variant="outlined"
        title={principal.kind === 'user' ? principal.user.email : undefined}
        sx={{ maxWidth: '100%' }}
      />
      {!isAssignable(principal) && (
        <Typography color="text.secondary" variant="caption">
          {t(
            principal.kind === 'team'
              ? 'principals.deletedTeam'
              : 'principals.inactiveUser',
          )}
        </Typography>
      )}
    </Stack>
  );
};

/** Plain-text form of an assignment, for headings and compact text. */
export const PrincipalText = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  const directory = usePrincipalDirectory(
    value !== null && value !== undefined,
  );
  if (value === null || value === undefined || value === '')
    return <>{t('views.notSet')}</>;
  return <>{resolvePrincipal(directory.data, value)?.label ?? String(value)}</>;
};

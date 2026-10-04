import { Chip, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { compactIconSize } from '../../components/iconSizes';
import { NotSetValue } from '../views/components/values/NotSetValue';
import { AssignedUserIcon, TeamIcon } from '../../components/systemIcons';
import { principalKinds } from './constants';
import { isAssignable, resolvePrincipal } from './principal';
import type { PrincipalKind } from './schemas';
import { usePrincipalDirectory } from './usePrincipalDirectory';

/** The icon of a user or a team at the compact size. */
export const PrincipalKindIcon = ({ kind }: { kind: PrincipalKind }) =>
  kind === principalKinds.team ? (
    <TeamIcon aria-hidden size={compactIconSize} />
  ) : (
    <AssignedUserIcon aria-hidden size={compactIconSize} />
  );

const isAssigned = (value: unknown) =>
  value !== null && value !== undefined && value !== '';

/**
 * An assigned user or team as a labelled chip. References the directory no
 * longer lists, or people who left, stay visible with an explanation.
 */
export const PrincipalValue = ({ value }: { value: unknown }) => {
  const { t } = useTranslation();
  const assigned = isAssigned(value);
  const directory = usePrincipalDirectory(assigned);
  if (!assigned) return <NotSetValue />;
  const principal = resolvePrincipal(directory.data, value);
  if (!principal) {
    // A reference is unknown only once the directory has loaded without it.
    if (directory.isLoadingError)
      return (
        <Typography color="error" variant="body2">
          {t('principals.loadFailed')}
        </Typography>
      );
    return (
      <Typography color="text.secondary" variant="body2">
        {directory.isSuccess
          ? t('principals.unknown', { value: String(value) })
          : t('principals.loading')}
      </Typography>
    );
  }
  return (
    <Stack spacing={0.5} sx={{ alignItems: 'flex-start' }}>
      <Chip
        icon={<PrincipalKindIcon kind={principal.kind} />}
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
  const assigned = isAssigned(value);
  const directory = usePrincipalDirectory(assigned);
  if (!assigned) return <>{t('views.notSet')}</>;
  return <>{resolvePrincipal(directory.data, value)?.label ?? String(value)}</>;
};

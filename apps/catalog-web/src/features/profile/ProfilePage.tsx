import { useQuery } from '@tanstack/react-query';
import { Alert, Paper, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { LanguageSwitcher } from '../../components/LanguageSwitcher';
import { TimeZonePreference } from './TimeZonePreference';

export const ProfilePage = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const account = session.data;
  return (
    <Stack spacing={3}>
      {session.isError && (
        <Alert severity="error">{session.error.message}</Alert>
      )}
      <Paper sx={{ p: 2 }}>
        <Typography variant="h6">{t('profile.accountDetails')}</Typography>
        <Typography>
          {t('profile.displayName', {
            value: account?.display_name ?? t('profile.notSet'),
          })}
        </Typography>
        <Typography>
          {t('profile.email', {
            value: account?.email ?? t('profile.loading'),
          })}
        </Typography>
        <Typography>
          {t('profile.userId', {
            value: account?.user_id ?? t('profile.loading'),
          })}
        </Typography>
        <Typography>
          {t('profile.activeWorkspace', {
            value: account?.workspace_id ?? t('profile.loading'),
          })}
        </Typography>
      </Paper>
      <Paper sx={{ p: 2 }}>
        <Stack direction="row" spacing={2} sx={{ alignItems: 'center' }}>
          <Typography variant="h6">{t('language.label')}</Typography>
          <LanguageSwitcher />
        </Stack>
      </Paper>
      <Paper sx={{ p: 2 }}>
        <TimeZonePreference disabled={!account} />
      </Paper>
    </Stack>
  );
};

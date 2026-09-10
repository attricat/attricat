import { useQuery } from '@tanstack/react-query';
import { Alert, Box, Button, Paper, Stack, Typography } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import { LanguageSwitcher } from '../../components/LanguageSwitcher';
import { PersonalTokens } from './PersonalTokens';

export const ProfilePage = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const account = session.data;
  return (
    <Box sx={{ maxWidth: 1000, mx: 'auto', p: 3 }}>
      <Typography variant="h4">{t('profile.title')}</Typography>
      {session.isError && (
        <Alert severity="error">{session.error.message}</Alert>
      )}
      <Stack spacing={3} sx={{ mt: 3 }}>
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
        <Box id="personal-api-tokens">
          <Stack
            direction="row"
            sx={{
              alignItems: 'center',
              justifyContent: 'space-between',
              mb: 1,
            }}
          >
            <Typography variant="h5">{t('profile.tokenList')}</Typography>
            {account?.capabilities?.tokens_manage === true && (
              <Button
                component={Link}
                to="/profile/personal-access-tokens"
                variant="contained"
              >
                {t('profile.createToken')}
              </Button>
            )}
          </Stack>
          <PersonalTokens
            canManage={account?.capabilities?.tokens_manage === true}
          />
        </Box>
      </Stack>
    </Box>
  );
};

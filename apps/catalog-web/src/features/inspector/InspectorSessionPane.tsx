import { Box, Link, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { personalApiTokensHref } from './constants';

export const InspectorSessionPane = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
    retry: false,
  });
  return (
    <Box sx={{ display: 'grid', gap: 0.5 }}>
      {session.isPending ? (
        <Typography variant="body2">
          {t('inspector.checkingSession')}
        </Typography>
      ) : session.data ? (
        <>
          <Typography variant="body2">
            {t('inspector.signedInAs', { email: session.data.email })}
          </Typography>
          <Typography color="text.secondary" variant="caption">
            {t('inspector.workspace', {
              workspace: session.data.login_identifier,
            })}
          </Typography>
          <Typography color="text.secondary" variant="caption">
            {t('inspector.tokenHelp')}
          </Typography>
          <Link href={personalApiTokensHref}>
            {t('inspector.manageTokens')}
          </Link>
        </>
      ) : (
        <Typography variant="body2">{t('inspector.notSignedIn')}</Typography>
      )}
    </Box>
  );
};

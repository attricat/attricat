import { Alert, Typography } from '@mui/material';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { apiErrorText } from '../../api/errors';
import { RouterButton } from '../../components/RouterLink';
import { AuthFormShell } from './AuthFormShell';
import { takeReturnLocation } from './returnLocation';
import { sessionOptions } from './queryOptions';

/**
 * Reading the return location forgets it, so finishing must happen once. The
 * page can remount while it runs, which a ref would not survive.
 */
let finishing = false;

/**
 * Where the identity provider's callback leaves the browser. The session
 * cookie already exists, so success only has to load it and move on; a failure
 * names just its error code.
 */
export const SingleSignOnCompletePage = ({ error }: { error?: string }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  useEffect(() => {
    if (error || finishing) return;
    finishing = true;
    void (async () => {
      queryClient.clear();
      await queryClient.fetchQuery(sessionOptions());
      await navigate({ replace: true, to: takeReturnLocation() ?? '/' });
    })().finally(() => {
      finishing = false;
    });
  }, [error, navigate, queryClient]);
  if (!error) return null;
  return (
    <AuthFormShell onSubmit={() => undefined}>
      <Typography component="h1" variant="h5">
        {t('auth.singleSignOnFailed')}
      </Typography>
      <Alert severity="error">
        {apiErrorText({
          code: error,
          message: t('auth.singleSignOnFailedDescription'),
        })}
      </Alert>
      <RouterButton to="/login" variant="outlined">
        {t('auth.backToSignIn')}
      </RouterButton>
    </AuthFormShell>
  );
};

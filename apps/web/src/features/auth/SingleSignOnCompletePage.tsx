import { Alert, Button } from '@mui/material';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { RotateCcwIcon } from 'lucide-react';
import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { apiErrorText } from '../../api/errors';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { RouterButton } from '../../components/RouterLink';
import { AuthFormShell, AuthFormTitle } from './AuthFormShell';
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
  const [attempt, setAttempt] = useState(0);
  const [loadError, setLoadError] = useState<Error | null>(null);
  useEffect(() => {
    if (error || finishing) return;
    finishing = true;
    void (async () => {
      queryClient.clear();
      await queryClient.fetchQuery(sessionOptions());
      await navigate({ replace: true, to: takeReturnLocation() ?? '/' });
    })()
      .catch((failure: unknown) =>
        setLoadError(
          failure instanceof Error ? failure : new Error(String(failure)),
        ),
      )
      .finally(() => {
        finishing = false;
      });
  }, [attempt, error, navigate, queryClient]);
  if (loadError)
    return (
      <AuthFormShell onSubmit={() => undefined}>
        <AuthFormTitle>{t('auth.singleSignOnFailed')}</AuthFormTitle>
        <ApiErrorAlert error={loadError} />
        <Button
          onClick={() => {
            setLoadError(null);
            setAttempt((current) => current + 1);
          }}
          startIcon={<RotateCcwIcon />}
          variant="contained"
        >
          {t('errors.retry')}
        </Button>
        <RouterButton to="/login" variant="outlined">
          {t('auth.backToSignIn')}
        </RouterButton>
      </AuthFormShell>
    );
  if (!error) return null;
  return (
    <AuthFormShell onSubmit={() => undefined}>
      <AuthFormTitle>{t('auth.singleSignOnFailed')}</AuthFormTitle>
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

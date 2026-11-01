import { Button } from '@mui/material';
import { useMutation } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { startSingleSignOn } from './api';
import { leaveForProvider } from './returnLocation';

/** Leaves the app for the workspace's identity provider. */
export const SingleSignOnButton = ({
  displayName,
  invitationSecret,
  loginIdentifier,
}: {
  displayName: string;
  /** Accepts this invitation by signing in. */
  invitationSecret?: string;
  loginIdentifier: string;
}) => {
  const { t } = useTranslation();
  // The page stays disabled until the browser leaves it.
  const start = useMutation({
    meta: { toast: false },
    mutationFn: () => startSingleSignOn(loginIdentifier, invitationSecret),
    onSuccess: ({ authorization_url }) => leaveForProvider(authorization_url),
  });
  return (
    <>
      <ApiErrorAlert error={start.error} />
      <Button
        disabled={start.isPending || start.isSuccess}
        onClick={() => start.mutate()}
        variant="contained"
      >
        {t('auth.continueWith', { provider: displayName })}
      </Button>
    </>
  );
};

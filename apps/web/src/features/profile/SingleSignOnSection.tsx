import { Button, Typography } from '@mui/material';
import { useMutation, useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { SingleSignOnIcon } from '../../components/systemIcons';
import { getIdentityLinkStatus, startIdentityLink } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { rememberReturnLocation } from '../auth/returnLocation';
import { ProfileSection } from './ProfileSection';

/** Shown only where the workspace has a provider to connect to. */
export const SingleSignOnSection = () => {
  const { t } = useTranslation();
  const status = useQuery({
    queryKey: authQueryKeys.identityLink(),
    queryFn: getIdentityLinkStatus,
  });
  const connect = useMutation({
    meta: { toast: false },
    mutationFn: startIdentityLink,
    onSuccess: ({ authorization_url }) => {
      rememberReturnLocation();
      window.location.assign(authorization_url);
    },
  });
  if (!status.data) return null;
  return (
    <ProfileSection
      description={t('profile.singleSignOnDescription')}
      icon={SingleSignOnIcon}
      title={t('profile.singleSignOn')}
    >
      <ApiErrorAlert error={connect.error} />
      {status.data.linked ? (
        <Typography>
          {t('profile.singleSignOnLinked', {
            provider: status.data.display_name,
          })}
        </Typography>
      ) : (
        <Button
          disabled={connect.isPending || connect.isSuccess}
          onClick={() => connect.mutate()}
          variant="outlined"
        >
          {t('profile.singleSignOnConnect', {
            provider: status.data.display_name,
          })}
        </Button>
      )}
    </ProfileSection>
  );
};

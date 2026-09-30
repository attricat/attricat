import { useQuery } from '@tanstack/react-query';
import { Button, Stack } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { PersonalTokens } from './PersonalTokens';

export const PersonalTokensPage = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canManage = session.data?.capabilities?.tokens_manage === true;
  return (
    <Stack spacing={2}>
      {canManage && (
        <Stack direction="row" sx={{ justifyContent: 'flex-end' }}>
          <Button
            component={Link}
            to="/profile/personal-access-tokens/new"
            variant="contained"
          >
            {t('profile.createToken')}
          </Button>
        </Stack>
      )}
      <PersonalTokens canManage={canManage} />
    </Stack>
  );
};

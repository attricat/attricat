import { useQuery } from '@tanstack/react-query';
import { Button, Stack, Typography } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { PlusIcon } from 'lucide-react';
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
    <Stack spacing={5}>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={4}
        sx={{ alignItems: { sm: 'center' }, justifyContent: 'space-between' }}
      >
        <Typography color="text.secondary" variant="body2">
          {t('profile.tokensDescription')}
        </Typography>
        {canManage && (
          <Button
            component={Link}
            startIcon={<PlusIcon />}
            sx={{
              alignSelf: { xs: 'flex-start', sm: 'center' },
              flexShrink: 0,
            }}
            to="/profile/personal-access-tokens/new"
            variant="contained"
          >
            {t('profile.createToken')}
          </Button>
        )}
      </Stack>
      <PersonalTokens canManage={canManage} />
    </Stack>
  );
};

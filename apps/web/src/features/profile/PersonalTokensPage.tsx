import { Stack, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { PlusIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { useCan } from '../auth/useCan';
import { PersonalTokens } from './PersonalTokens';
import { capabilities } from '../auth/capabilities';
import { sessionOptions } from '../auth/queryOptions';
import { RouterButton } from '../../components/RouterLink';

export const PersonalTokensPage = () => {
  const { t } = useTranslation();
  const session = useQuery(sessionOptions());
  const canManage = useCan(capabilities.tokensManage);
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
          <RouterButton
            startIcon={<PlusIcon />}
            sx={{
              alignSelf: { xs: 'flex-start', sm: 'center' },
              flexShrink: 0,
            }}
            to="/profile/personal-access-tokens/new"
            variant="contained"
          >
            {t('profile.createToken')}
          </RouterButton>
        )}
      </Stack>
      {/* The capability is unknown until the session loads. */}
      {!session.isPending && <PersonalTokens canManage={canManage} />}
    </Stack>
  );
};

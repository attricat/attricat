import { Alert, Box, Button, Stack, Typography } from '@mui/material';
import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

const RecoveryScreen = ({
  actions,
  description,
  title,
}: {
  actions: ReactNode;
  description: string;
  title: string;
}) => (
  <Box
    component="main"
    sx={{
      alignItems: 'center',
      display: 'flex',
      justifyContent: 'center',
      minHeight: '100dvh',
      p: 3,
    }}
  >
    <Stack spacing={3} sx={{ maxWidth: 560, width: '100%' }}>
      <Typography component="h1" variant="h4">
        {title}
      </Typography>
      <Alert severity="error">{description}</Alert>
      <Stack direction={{ sm: 'row' }} spacing={2}>
        {actions}
      </Stack>
    </Stack>
  </Box>
);

export const RouterErrorScreen = ({ onRetry }: { onRetry: () => void }) => {
  const { t } = useTranslation();

  return (
    <RecoveryScreen
      actions={
        <>
          <Button onClick={onRetry} variant="contained">
            {t('errors.retry')}
          </Button>
          <Button href="/" variant="outlined">
            {t('errors.goHome')}
          </Button>
        </>
      }
      description={t('errors.unexpectedDescription')}
      title={t('errors.unexpectedTitle')}
    />
  );
};

export const NotFoundScreen = () => {
  const { t } = useTranslation();

  return (
    <RecoveryScreen
      actions={
        <Button href="/" variant="contained">
          {t('errors.goHome')}
        </Button>
      }
      description={t('errors.notFoundDescription')}
      title={t('errors.notFoundTitle')}
    />
  );
};

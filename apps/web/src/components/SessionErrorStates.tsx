import { Alert, Box, Button } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RotateCcwIcon } from 'lucide-react';

export const SessionErrorState = ({
  onRetry,
  onSignOut,
}: {
  onRetry: () => void;
  onSignOut: () => void;
}) => {
  const { t } = useTranslation();

  return (
    <Box
      sx={{
        alignItems: 'center',
        display: 'flex',
        height: '100dvh',
        justifyContent: 'center',
        p: 3,
      }}
    >
      <Alert
        action={
          <Box sx={{ display: 'flex', gap: 1 }}>
            <Button
              color="inherit"
              onClick={onRetry}
              size="small"
              startIcon={<RotateCcwIcon />}
            >
              {t('auth.retrySession')}
            </Button>
            <Button color="inherit" onClick={onSignOut} size="small">
              {t('navigation.signOut')}
            </Button>
          </Box>
        }
        role="alert"
        severity="error"
      >
        {t('auth.sessionCheckFailed')}
      </Alert>
    </Box>
  );
};

export const SignOutErrorState = ({ onRetry }: { onRetry: () => void }) => {
  const { t } = useTranslation();

  return (
    <Alert
      action={
        <Button
          color="inherit"
          onClick={onRetry}
          size="small"
          startIcon={<RotateCcwIcon />}
        >
          {t('auth.retrySignOut')}
        </Button>
      }
      role="alert"
      severity="error"
    >
      {t('auth.signOutFailed')}
    </Alert>
  );
};

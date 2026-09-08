import { Alert, AlertTitle, Button } from '@mui/material';
import { useTranslation } from 'react-i18next';

export const QueryErrorNotice = ({
  error,
  isRetrying = false,
  onRetry,
}: {
  error: Error | null | undefined;
  isRetrying?: boolean;
  onRetry: () => void;
}) => {
  const { t } = useTranslation();

  if (!error) return null;

  return (
    <Alert
      action={
        <Button
          color="inherit"
          disabled={isRetrying}
          onClick={onRetry}
          size="small"
        >
          {isRetrying ? t('common.loading') : t('errors.retry')}
        </Button>
      }
      severity="error"
    >
      <AlertTitle>{t('errors.queryTitle')}</AlertTitle>
      {t('errors.queryDescription')}
    </Alert>
  );
};

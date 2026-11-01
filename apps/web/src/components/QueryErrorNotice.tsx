import { Alert, AlertTitle, Button } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { RotateCcwIcon } from 'lucide-react';
import { errorMessage } from '../api/errors';
import { isRefusedRequest } from '../api/queryRetries';

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
  // A refused request, such as a missing resource, says why and would fail
  // again; anything else is most likely a connection failure.
  const refused = isRefusedRequest(error);

  return (
    <Alert
      action={
        !refused && (
          <Button
            color="inherit"
            disabled={isRetrying}
            onClick={onRetry}
            size="small"
            startIcon={<RotateCcwIcon />}
          >
            {isRetrying ? t('common.loading') : t('errors.retry')}
          </Button>
        )
      }
      severity="error"
    >
      <AlertTitle>{t('errors.queryTitle')}</AlertTitle>
      {refused ? errorMessage(error) : t('errors.queryDescription')}
    </Alert>
  );
};

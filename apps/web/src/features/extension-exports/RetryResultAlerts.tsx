import { Alert, Stack, type SxProps, type Theme } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import type { useRetryExports } from './useRetryExports';

/** A failed retry, or the jobs a retry could not start. */
export const RetryResultAlerts = ({
  retry,
  sx,
}: {
  retry: ReturnType<typeof useRetryExports>;
  sx?: SxProps<Theme>;
}) => {
  const { t } = useTranslation();
  if (!retry.error && !retry.data?.job_errors.length) return null;
  return (
    <Stack spacing={2} sx={sx}>
      <ApiErrorAlert error={retry.error} />
      {!!retry.data?.job_errors.length && (
        <Alert severity="warning">
          {t('extensionExports.retryJobErrors', {
            messages: retry.data.job_errors
              .map((error) => error.message)
              .join('; '),
          })}
        </Alert>
      )}
    </Stack>
  );
};

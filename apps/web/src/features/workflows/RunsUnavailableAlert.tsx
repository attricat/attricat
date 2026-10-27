import { Alert, type SxProps, type Theme } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { errorMessage } from '../../api/errors';

/** Workflow runs failed to load; the workflows themselves still show. */
export const RunsUnavailableAlert = ({
  error,
  sx,
}: {
  error: Error | null;
  sx?: SxProps<Theme>;
}) => {
  const { t } = useTranslation();
  if (!error) return null;
  return (
    <Alert severity="warning" sx={sx}>
      {t('workflows.runsUnavailable', { message: errorMessage(error) })}
    </Alert>
  );
};

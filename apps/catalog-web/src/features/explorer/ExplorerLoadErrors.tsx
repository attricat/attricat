import { Alert, Stack } from '@mui/material';
import type { UseQueryResult } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';

type RetryableQuery = Pick<UseQueryResult, 'error' | 'isFetching' | 'refetch'>;

type Props = {
  blueprintMissing: boolean;
  blueprints: RetryableQuery;
  selectedBlueprint: RetryableQuery;
};

export const ExplorerLoadErrors = ({
  blueprintMissing,
  blueprints,
  selectedBlueprint,
}: Props) => {
  const { t } = useTranslation();
  if (!blueprints.error && !selectedBlueprint.error && !blueprintMissing)
    return null;
  return (
    <Stack spacing={2} sx={{ mb: 2 }}>
      <QueryErrorNotice
        error={blueprints.error}
        isRetrying={blueprints.isFetching}
        onRetry={() => void blueprints.refetch()}
      />
      <QueryErrorNotice
        error={selectedBlueprint.error}
        isRetrying={selectedBlueprint.isFetching}
        onRetry={() => void selectedBlueprint.refetch()}
      />
      {blueprintMissing && (
        <Alert severity="error">{t('explorer.blueprintNotFound')}</Alert>
      )}
    </Stack>
  );
};

import { Chip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { recordExportStatus } from './api';
import { exportProblemStatuses } from './constants';
import { extensionExportQueryKeys } from './queryKeys';

/**
 * Warns that a record's export to a destination was rejected, is uncertain
 * or is being retried, and opens its export status on the record.
 */
export const RecordExportStatusChip = ({
  recordId,
  onOpen,
}: {
  recordId: string;
  onOpen: () => void;
}) => {
  const { t } = useTranslation();
  const status = useQuery({
    queryKey: extensionExportQueryKeys.record(recordId),
    queryFn: () => recordExportStatus(recordId),
  });
  const problems =
    status.data?.statuses.filter((mapping) =>
      exportProblemStatuses.includes(mapping.status),
    ).length ?? 0;
  if (!problems) return null;
  return (
    <Chip
      clickable
      color="warning"
      label={t('extensionExports.recordProblems', { count: problems })}
      size="small"
      onClick={onOpen}
    />
  );
};

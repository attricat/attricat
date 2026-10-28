import { Chip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { recordFindingCountOptions } from '../../rules/queryOptions';

/**
 * Warns about unresolved data quality findings for a record and opens their
 * list on the record.
 */
export const DataQualityFindingsChip = ({
  recordId,
  onOpen,
}: {
  recordId: string;
  onOpen: () => void;
}) => {
  const { t } = useTranslation();
  const count = useQuery(recordFindingCountOptions(recordId));
  if (!count.data) return null;
  return (
    <Chip
      clickable
      color="warning"
      label={t('records.dataQualityFindings', { count: count.data })}
      size="small"
      onClick={onOpen}
    />
  );
};

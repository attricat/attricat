import { Chip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { recordFindingCountOptions } from '../../rules/queryOptions';
import { useCan } from '../../auth/useCan';
import { capabilities } from '../../auth/capabilities';

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
  const canReadRules = useCan(capabilities.rulesRead);
  const count = useQuery({
    ...recordFindingCountOptions(recordId),
    enabled: canReadRules,
  });
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

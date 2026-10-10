import { Chip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { listFindings } from '../../rules/api';
import { FINDING_STATE_RESOLVED } from '../../rules/constants';
import { ruleQueryKeys } from '../../rules/queryKeys';

/** Warns about unresolved data quality findings for a record. */
export const DataQualityFindingsChip = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const findings = useQuery({
    queryKey: ruleQueryKeys.findings(recordId),
    queryFn: () => listFindings(recordId),
  });
  const unresolvedCount =
    findings.data?.filter((finding) => finding.state !== FINDING_STATE_RESOLVED)
      .length ?? 0;
  if (unresolvedCount === 0) return null;
  return (
    <Chip
      color="warning"
      label={t('records.dataQualityFindings', { count: unresolvedCount })}
      size="small"
    />
  );
};

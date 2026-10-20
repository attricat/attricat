import { Chip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { listFindings } from '../../rules/api';
import {
  FINDING_STATE_RESOLVED,
  recordFindingsPanelId,
} from '../../rules/constants';
import { ruleQueryKeys } from '../../rules/queryKeys';
import { useCan } from '../../auth/useCan';

/**
 * Warns about unresolved data quality findings for a record and moves to
 * their list on the record.
 */
export const DataQualityFindingsChip = ({ recordId }: { recordId: string }) => {
  const { t } = useTranslation();
  const canReadRules = useCan('rules_read');
  const findings = useQuery({
    queryKey: ruleQueryKeys.findings(recordId),
    queryFn: () => listFindings(recordId),
    enabled: canReadRules,
  });
  const unresolvedCount =
    findings.data?.filter((finding) => finding.state !== FINDING_STATE_RESOLVED)
      .length ?? 0;
  if (unresolvedCount === 0) return null;
  return (
    <Chip
      clickable
      color="warning"
      label={t('records.dataQualityFindings', { count: unresolvedCount })}
      size="small"
      onClick={() => {
        const panel = document.getElementById(recordFindingsPanelId(recordId));
        panel?.scrollIntoView({ behavior: 'smooth', block: 'center' });
        panel?.focus({ preventScroll: true });
      }}
    />
  );
};

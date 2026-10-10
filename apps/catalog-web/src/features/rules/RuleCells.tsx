import { Box } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { SHORT_RECORD_ID_LENGTH } from './constants';
import { ruleRevisionKey, type RuleRevisions } from './ruleRevisions';

export const RuleRevisionCell = ({
  revisions,
  ruleId,
  version,
}: {
  revisions: RuleRevisions;
  ruleId: string;
  version: number;
}) => {
  const { t } = useTranslation();
  const rule = revisions.get(ruleRevisionKey(ruleId, version));
  return rule
    ? t('rules.ruleRevision', { name: rule.name, version })
    : t('rules.unknownRule', {
        id: ruleId.slice(0, SHORT_RECORD_ID_LENGTH),
        version,
      });
};

/** Links a record by its display label, or by a short ID until one loads. */
export const RecordIdLink = ({
  recordId,
  label,
}: {
  recordId: string;
  label: string | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <Link params={{ recordId }} title={recordId} to="/records/$recordId">
      {label ?? (
        <Box
          aria-label={t('rules.recordWithoutLabel', { id: recordId })}
          component="span"
          sx={{ fontFamily: 'monospace' }}
        >
          {recordId.slice(0, SHORT_RECORD_ID_LENGTH)}
        </Box>
      )}
    </Link>
  );
};

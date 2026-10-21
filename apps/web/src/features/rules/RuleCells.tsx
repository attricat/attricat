import { Box, Chip } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import type { Rule } from './api';
import { RULE_STATUS_PUBLISHED, SHORT_RECORD_ID_LENGTH } from './constants';
import { ruleRevisionKey, type RuleRevisions } from './ruleRevisions';

/** Names a rule revision, linking to its definition once it is known. */
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
  return rule ? (
    <Link
      params={{ ruleId, version: String(version) }}
      to="/manage/rules/$ruleId/revisions/$version"
    >
      {t('rules.ruleRevision', { name: rule.name, version })}
    </Link>
  ) : (
    t('rules.unknownRule', {
      id: ruleId.slice(0, SHORT_RECORD_ID_LENGTH),
      version,
    })
  );
};

/** Whether a rule revision is enabled, published but disabled, or a draft. */
export const RuleLifecycleChip = ({ rule }: { rule: Rule }) => {
  const { t } = useTranslation();
  // Rows are revisions; only the enabled one is enabled.
  const enabled = rule.enabled_version === rule.version;
  return (
    <Chip
      color={enabled ? 'success' : 'default'}
      label={
        enabled
          ? t('rules.enabled')
          : rule.status === RULE_STATUS_PUBLISHED
            ? t('rules.disabled')
            : t('rules.draft')
      }
      size="small"
    />
  );
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

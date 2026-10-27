import { Chip, Tooltip, useTheme } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { TriangleAlertIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Rule, RuleDetails } from './api';
import { shortId } from '../../components/shortId';
import { compactIconSize } from '../../components/iconSizes';
import { ruleRoutes } from './constants';
import type { RulesById } from './rulesById';

/**
 * Names a rule and the revision a run or finding used, linking to that
 * revision in the rule's changelog.
 */
export const RuleRevisionCell = ({
  rules,
  ruleId,
  version,
}: {
  rules: RulesById;
  ruleId: string;
  version: number;
}) => {
  const { t } = useTranslation();
  const rule = rules.get(ruleId);
  return rule ? (
    <Link
      params={{ ruleId }}
      search={{ to: version }}
      to={ruleRoutes.changelog}
    >
      {t('rules.ruleRevision', { name: rule.name, version })}
    </Link>
  ) : (
    t('rules.unknownRule', {
      id: shortId(ruleId),
      version,
    })
  );
};

/** Whether a rule is enabled. */
export const RuleLifecycleChip = ({ rule }: { rule: Rule }) => {
  const { t } = useTranslation();
  return (
    <Chip
      color={rule.enabled ? 'success' : 'default'}
      label={rule.enabled ? t('rules.enabled') : t('rules.disabled')}
      size="small"
    />
  );
};

/**
 * Warns that a rule skips the records of some revisions of its blueprint,
 * naming them.
 */
export const RuleSkipsRevisionsIcon = ({ rule }: { rule: RuleDetails }) => {
  const { t } = useTranslation();
  const { palette } = useTheme();
  if (rule.not_applicable.length === 0) return null;
  const label = t('rules.skipsRevisions', {
    count: rule.not_applicable.length,
    versions: rule.not_applicable
      .map((revision) => revision.blueprint_version)
      .join(', '),
  });
  return (
    <Tooltip title={label}>
      <TriangleAlertIcon
        aria-label={label}
        color={palette.warning.main}
        role="img"
        size={compactIconSize}
      />
    </Tooltip>
  );
};

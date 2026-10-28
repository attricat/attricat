import { Chip, Stack, Tooltip, Typography, useTheme } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { TriangleAlertIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { Rule, RuleDetails } from './api';
import { shortId } from '../../components/shortId';
import { compactIconSize } from '../../components/iconSizes';
import { findingStates, ruleRoutes } from './constants';
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

/**
 * A rule's open findings, linking to them on the findings page, and how
 * many of the rest someone acknowledged.
 */
export const RuleFindingCounts = ({ rule }: { rule: RuleDetails }) => {
  const { t } = useTranslation();
  const open = rule.open_findings ?? 0;
  const acknowledged = rule.acknowledged_findings ?? 0;
  return (
    <Stack spacing={0.25}>
      <Typography variant="body2">
        {open > 0 ? (
          <Link
            search={{ rule: rule.id, state: findingStates.open }}
            to={ruleRoutes.findings}
          >
            {t('rules.openFindings', { count: open })}
          </Link>
        ) : (
          t('rules.openFindings', { count: open })
        )}
      </Typography>
      {acknowledged > 0 && (
        <Typography color="text.secondary" variant="caption">
          <Link
            search={{ rule: rule.id, state: findingStates.acknowledged }}
            to={ruleRoutes.findings}
          >
            {t('rules.acknowledgedFindings', { count: acknowledged })}
          </Link>
        </Typography>
      )}
    </Stack>
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

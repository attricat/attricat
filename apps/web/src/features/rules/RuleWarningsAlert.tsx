import { Alert, Box, type SxProps, type Theme } from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import type { RuleWarning } from './api';
import { ruleRoutes } from './constants';

/**
 * The enabled rules that a newly published blueprint revision no longer fits.
 * Publishing went ahead; these rules skip the revision's records.
 */
export const RuleWarningsAlert = ({
  sx,
  version,
  warnings,
}: {
  sx?: SxProps<Theme>;
  /** The published blueprint revision. */
  version: number;
  warnings: readonly RuleWarning[] | undefined;
}) => {
  const { t } = useTranslation();
  if (!warnings?.length) return null;
  return (
    <Alert severity="warning" sx={sx}>
      {t('rules.publishWarnings.title', { count: warnings.length, version })}
      <Box component="ul" sx={{ mb: 0, mt: 0.5, pl: 2.5 }}>
        {warnings.map((warning) => (
          <li key={warning.rule_id}>
            <Link params={{ ruleId: warning.rule_id }} to={ruleRoutes.rule}>
              {warning.name}
            </Link>
            {': '}
            {t(`rules.inapplicability.${warning.reason}`)}
          </li>
        ))}
      </Box>
    </Alert>
  );
};

import {
  Alert,
  Box,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { listRuleRuns } from './api';
import { EntityIdLink, RuleRevisionCell } from './RuleCells';
import type { RuleRevisions } from './ruleRevisions';

export const RunsSection = ({
  entityLabels,
  error,
  revisions,
  runs,
}: {
  entityLabels: ReadonlyMap<string, string>;
  error: boolean;
  revisions: RuleRevisions;
  runs: Awaited<ReturnType<typeof listRuleRuns>> | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('rules.errors.runs')}
        </Alert>
      )}
      <Paper component="section" sx={{ mt: 3 }}>
        <Box sx={{ p: 2 }}>
          <Typography variant="h6">{t('rules.runHistory')}</Typography>
        </Box>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('rules.columns.rule')}</TableCell>
              <TableCell>{t('rules.columns.scope')}</TableCell>
              <TableCell>{t('rules.columns.source')}</TableCell>
              <TableCell>{t('rules.columns.status')}</TableCell>
              <TableCell>{t('rules.columns.evaluated')}</TableCell>
              <TableCell>{t('rules.columns.findings')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {(runs ?? []).map((item) => (
              <TableRow key={item.id}>
                <TableCell>
                  <RuleRevisionCell
                    revisions={revisions}
                    ruleId={item.rule_id}
                    version={item.rule_version}
                  />
                </TableCell>
                <TableCell>
                  {item.scope_entity_id ? (
                    <EntityIdLink
                      entityId={item.scope_entity_id}
                      label={entityLabels.get(item.scope_entity_id)}
                    />
                  ) : (
                    t('rules.allEntities')
                  )}
                </TableCell>
                <TableCell>
                  {item.dry_run
                    ? t('rules.dryRunSuffix', { source: item.source })
                    : item.source}
                </TableCell>
                <TableCell>
                  {t(`rules.runStatuses.${item.status}`, {
                    defaultValue: item.status,
                  })}
                </TableCell>
                <TableCell>{item.candidates_evaluated}</TableCell>
                <TableCell>{item.findings_created}</TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Paper>
    </>
  );
};

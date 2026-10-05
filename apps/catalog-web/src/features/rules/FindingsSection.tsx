import {
  Alert,
  Box,
  Button,
  Chip,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { listFindings } from './api';
import { EntityIdLink, RuleRevisionCell } from './RuleCells';
import type { RuleRevisions } from './ruleRevisions';
import {
  ERROR_FINDING_SEVERITIES,
  FINDING_STATE_OPEN,
  FINDING_STATE_RESOLVED,
} from './constants';

export const FindingsSection = ({
  acknowledging,
  canManage,
  error,
  findings,
  entityLabels,
  onAcknowledge,
  revisions,
}: {
  acknowledging: boolean;
  canManage: boolean;
  error: boolean;
  findings: Awaited<ReturnType<typeof listFindings>> | undefined;
  entityLabels: ReadonlyMap<string, string>;
  onAcknowledge: (id: string) => void;
  revisions: RuleRevisions;
}) => {
  const { t } = useTranslation();
  return (
    <>
      {error && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('rules.errors.findings')}
        </Alert>
      )}
      <Paper component="section" sx={{ mt: 3 }}>
        <Box sx={{ p: 2 }}>
          <Typography variant="h6">{t('rules.activeFindings')}</Typography>
        </Box>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('rules.columns.severity')}</TableCell>
              <TableCell>{t('rules.columns.finding')}</TableCell>
              <TableCell>{t('rules.columns.rule')}</TableCell>
              <TableCell>{t('rules.columns.entity')}</TableCell>
              <TableCell>{t('rules.columns.state')}</TableCell>
              <TableCell />
            </TableRow>
          </TableHead>
          <TableBody>
            {(findings ?? [])
              .filter((item) => item.state !== FINDING_STATE_RESOLVED)
              .map((finding) => (
                <TableRow key={finding.id}>
                  <TableCell>
                    <Chip
                      color={
                        ERROR_FINDING_SEVERITIES.includes(finding.severity)
                          ? 'error'
                          : 'warning'
                      }
                      label={t(`rules.severities.${finding.severity}`, {
                        defaultValue: finding.severity,
                      })}
                      size="small"
                    />
                  </TableCell>
                  <TableCell>{finding.message}</TableCell>
                  <TableCell>
                    <RuleRevisionCell
                      revisions={revisions}
                      ruleId={finding.rule_id}
                      version={finding.rule_version}
                    />
                  </TableCell>
                  <TableCell>
                    <EntityIdLink
                      entityId={finding.entity_id}
                      label={entityLabels.get(finding.entity_id)}
                    />
                  </TableCell>
                  <TableCell>
                    {t(`rules.findingStates.${finding.state}`, {
                      defaultValue: finding.state,
                    })}
                  </TableCell>
                  <TableCell>
                    {canManage && finding.state === FINDING_STATE_OPEN && (
                      <Button
                        disabled={acknowledging}
                        size="small"
                        onClick={() => onAcknowledge(finding.id)}
                      >
                        {t('rules.acknowledge')}
                      </Button>
                    )}
                  </TableCell>
                </TableRow>
              ))}
          </TableBody>
        </Table>
      </Paper>
    </>
  );
};

import {
  Box,
  Button,
  FormControlLabel,
  Paper,
  Stack,
  Switch,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { Link } from '@tanstack/react-router';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { FlaskConicalIcon, PlayIcon } from 'lucide-react';
import type { Rule } from './api';
import { revisionStatuses } from '../../api/revisionStatus';
import { RuleLifecycleChip } from './RuleCells';

export const RulesSection = ({
  canManage,
  error,
  onRetry,
  onRun,
  onToggle,
  running,
  rules,
}: {
  canManage: boolean;
  /** The section's load failure, offered for retry. */
  error: Error | null;
  onRetry: () => void;
  onRun: (rule: Rule, dryRun: boolean) => void;
  onToggle: (rule: Rule, enabled: boolean) => void;
  running: boolean;
  rules: readonly Rule[] | undefined;
}) => {
  const { t } = useTranslation();
  return (
    <>
      {error && (
        <Box sx={{ mt: 3 }}>
          <QueryErrorNotice error={error} onRetry={onRetry} />
        </Box>
      )}
      <Paper component="section" sx={{ mt: 3 }}>
        <Box sx={{ overflowX: 'auto' }}>
          <Table size="small">
            <TableHead>
              <TableRow>
                <TableCell>{t('rules.columns.rule')}</TableCell>
                <TableCell>{t('rules.columns.revision')}</TableCell>
                <TableCell>{t('rules.columns.lifecycle')}</TableCell>
                <TableCell>{t('rules.columns.actions')}</TableCell>
              </TableRow>
            </TableHead>
            <TableBody>
              {(rules ?? []).map((rule) => {
                // Rows are revisions; only the enabled one is enabled.
                const enabled = rule.enabled_version === rule.version;
                const lifecycle = <RuleLifecycleChip rule={rule} />;
                return (
                  <TableRow key={`${rule.id}-${rule.version}`}>
                    <TableCell>
                      <Stack spacing={0.25}>
                        <Typography>
                          <Link
                            params={{
                              ruleId: rule.id,
                              version: String(rule.version),
                            }}
                            to="/manage/rules/$ruleId/revisions/$version"
                          >
                            {rule.name}
                          </Link>
                        </Typography>
                        <Typography color="text.secondary" variant="caption">
                          {rule.code}
                        </Typography>
                      </Stack>
                    </TableCell>
                    <TableCell>v{rule.version}</TableCell>
                    <TableCell>
                      {canManage &&
                      rule.status === revisionStatuses.published ? (
                        <FormControlLabel
                          control={
                            <Switch
                              checked={enabled}
                              disabled={running}
                              onChange={(_, checked) => onToggle(rule, checked)}
                              slotProps={{
                                input: {
                                  'aria-label': t('rules.toggleRule', {
                                    name: rule.name,
                                    version: rule.version,
                                  }),
                                },
                              }}
                            />
                          }
                          label={lifecycle}
                        />
                      ) : (
                        lifecycle
                      )}
                    </TableCell>
                    <TableCell>
                      {canManage && (
                        <>
                          <Button
                            disabled={running}
                            size="small"
                            onClick={() => onRun(rule, true)}
                            startIcon={<FlaskConicalIcon />}
                          >
                            {t('rules.dryRun')}
                          </Button>
                          <Button
                            disabled={running}
                            size="small"
                            onClick={() => onRun(rule, false)}
                            startIcon={<PlayIcon />}
                          >
                            {t('rules.runNow')}
                          </Button>
                        </>
                      )}
                    </TableCell>
                  </TableRow>
                );
              })}
            </TableBody>
          </Table>
        </Box>
      </Paper>
    </>
  );
};

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
import { EmptyState } from '../../components/EmptyState';
import { RuleIcon } from '../../components/systemIcons';
import { Timestamp } from '../../time/Timestamp';
import { FlaskConicalIcon, PlayIcon } from 'lucide-react';
import type { Blueprint } from '../blueprints/api';
import { lexiconText } from '../lexicon/lexicon';
import type { RuleDetails } from './api';
import { ruleRoutes } from './constants';
import {
  RuleFindingCounts,
  RuleLifecycleChip,
  RuleSkipsRevisionsIcon,
} from './RuleCells';
import { FindingSeverityChip } from './FindingSeverityChip';
import { ruleSeverity } from './ruleSeverity';

/** The workspace's rules, one row each, with their switches and runs. */
export const RulesSection = ({
  blueprints,
  canManage,
  error,
  onRetry,
  onRun,
  onToggle,
  running,
  rules,
}: {
  /** The workspace's blueprints, to name each rule's blueprint. */
  blueprints: readonly Blueprint[];
  canManage: boolean;
  /** The section's load failure, offered for retry. */
  error: Error | null;
  onRetry: () => void;
  onRun: (rule: RuleDetails, dryRun: boolean) => void;
  onToggle: (rule: RuleDetails, enabled: boolean) => void;
  running: boolean;
  rules: readonly RuleDetails[] | undefined;
}) => {
  const { t } = useTranslation();
  const blueprintName = (id: string) => {
    const blueprint = blueprints.find((candidate) => candidate.id === id);
    return blueprint ? lexiconText(blueprint.name) : undefined;
  };
  return (
    <>
      {error && (
        <Box sx={{ mt: 3 }}>
          <QueryErrorNotice error={error} onRetry={onRetry} />
        </Box>
      )}
      <Paper component="section" sx={{ mt: 3 }}>
        {rules?.length === 0 ? (
          <EmptyState
            description={t('rules.emptyDescription')}
            icon={RuleIcon}
            title={t('rules.empty')}
          />
        ) : (
          <Box sx={{ overflowX: 'auto' }}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>{t('rules.columns.rule')}</TableCell>
                  <TableCell>{t('rules.columns.blueprint')}</TableCell>
                  <TableCell>{t('rules.columns.severity')}</TableCell>
                  <TableCell>{t('rules.columns.openFindings')}</TableCell>
                  <TableCell>{t('rules.columns.changed')}</TableCell>
                  <TableCell>{t('rules.columns.lifecycle')}</TableCell>
                  <TableCell>{t('rules.columns.actions')}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {(rules ?? []).map((rule) => {
                  const lifecycle = <RuleLifecycleChip rule={rule} />;
                  const severity = ruleSeverity(rule);
                  return (
                    <TableRow key={rule.id}>
                      <TableCell>
                        <Stack spacing={0.25}>
                          <Stack
                            direction="row"
                            spacing={1}
                            sx={{ alignItems: 'center' }}
                          >
                            <Typography>
                              <Link
                                params={{ ruleId: rule.id }}
                                to={ruleRoutes.rule}
                              >
                                {rule.name}
                              </Link>
                            </Typography>
                            <RuleSkipsRevisionsIcon rule={rule} />
                          </Stack>
                          <Typography color="text.secondary" variant="caption">
                            {rule.code}
                          </Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>
                        {blueprintName(rule.blueprint_id) ?? (
                          <Typography color="text.secondary" variant="body2">
                            {t('rules.unknownBlueprint')}
                          </Typography>
                        )}
                      </TableCell>
                      <TableCell>
                        {severity && (
                          <FindingSeverityChip severity={severity} />
                        )}
                      </TableCell>
                      <TableCell>
                        <RuleFindingCounts rule={rule} />
                      </TableCell>
                      <TableCell>
                        <Stack spacing={0.25}>
                          <Typography variant="body2">
                            {t('rules.revisionLabel', {
                              version: rule.version,
                            })}
                          </Typography>
                          <Typography color="text.secondary" variant="caption">
                            <Timestamp relative value={rule.updated_at} />
                          </Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>
                        {canManage ? (
                          <FormControlLabel
                            control={
                              <Switch
                                checked={rule.enabled}
                                disabled={running}
                                onChange={(_, checked) =>
                                  onToggle(rule, checked)
                                }
                                slotProps={{
                                  input: {
                                    'aria-label': t('rules.toggleRule', {
                                      name: rule.name,
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
        )}
      </Paper>
    </>
  );
};

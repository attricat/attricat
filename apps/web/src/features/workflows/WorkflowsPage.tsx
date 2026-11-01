import { ActionsHeaderCell } from '../../components/ActionsHeaderCell';
import { latestRevisions } from '../../api/revisionStatus';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Box,
  Chip,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { RouterButton } from '../../components/RouterLink';
import { useTranslation } from 'react-i18next';
import { PlusIcon } from 'lucide-react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useCan } from '../auth/useCan';
import { workflowRoutes } from './constants';
import { RevisionStatusChip } from '../../components/RevisionStatusChip';
import { Timestamp } from '../../time/Timestamp';
import { EmptyState } from '../../components/EmptyState';
import { WorkflowIcon } from '../../components/systemIcons';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { workflowListOptions, workflowRunListOptions } from './queryOptions';
import { runStatuses } from '../../api/runStatus';
import { RunStatusChip } from '../../components/RunStatusChip';
import { RunsUnavailableAlert } from './RunsUnavailableAlert';
import { LoadingState } from '../../components/LoadingState';
import type { WorkflowRun } from './schemas';

export const WorkflowsPage = () => {
  const { t } = useTranslation();
  const canManage = useCan(capabilities.workflowsManage);
  const workflows = useQuery(workflowListOptions());
  const runs = useQuery(workflowRunListOptions());
  const families = latestRevisions(
    workflows.data ?? [],
    (workflow) => workflow.id,
  );
  const runSummaries = new Map<
    string,
    {
      deadLetters: number;
      latest: WorkflowRun;
    }
  >();
  for (const run of runs.data ?? []) {
    const summary = runSummaries.get(run.workflow_id) ?? {
      deadLetters: 0,
      latest: run,
    };
    if (run.status === runStatuses.deadLetter) summary.deadLetters += 1;
    if (run.created_at > summary.latest.created_at) summary.latest = run;
    runSummaries.set(run.workflow_id, summary);
  }

  return (
    <PageContainer>
      <PageHeader
        actions={
          canManage ? (
            <RouterButton
              startIcon={<PlusIcon />}
              to={workflowRoutes.create}
              variant="contained"
            >
              {t('workflows.newWorkflow')}
            </RouterButton>
          ) : undefined
        }
        description={t('workflows.description')}
        icon={WorkflowIcon}
        title={t('workflows.title')}
      />
      <ApiErrorAlert error={workflows.error} sx={{ mt: 3 }} />
      <RunsUnavailableAlert error={runs.error} sx={{ mt: 3 }} />
      {workflows.isPending && (
        <LoadingState sx={{ mt: 3 }}>{t('workflows.loading')}</LoadingState>
      )}
      {workflows.data && (
        <Paper component="section" sx={{ mt: 3 }}>
          <Box sx={{ overflowX: 'auto' }}>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>{t('workflows.workflow')}</TableCell>
                  <TableCell>{t('workflows.lifecycle')}</TableCell>
                  <TableCell>{t('workflows.currentRevision')}</TableCell>
                  <TableCell>{t('workflows.latestRun')}</TableCell>
                  <ActionsHeaderCell />
                </TableRow>
              </TableHead>
              <TableBody>
                {families.map((workflow) => {
                  const summary = runSummaries.get(workflow.id);
                  const enabled = workflow.enabled_version !== null;
                  return (
                    <TableRow hover key={workflow.id}>
                      <TableCell>
                        <Stack spacing={0.25}>
                          <Link
                            params={{ workflowId: workflow.id }}
                            to={workflowRoutes.detail}
                          >
                            {workflow.name}
                          </Link>
                          <Typography color="text.secondary" variant="caption">
                            {workflow.code}
                          </Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>
                        <Chip
                          color={enabled ? 'success' : 'default'}
                          label={
                            enabled
                              ? t('workflows.enabled')
                              : t('workflows.disabled')
                          }
                          size="small"
                        />
                      </TableCell>
                      <TableCell>
                        <Stack direction="row" spacing={1}>
                          <RevisionStatusChip
                            size="small"
                            status={workflow.status}
                          />
                          <Typography>v{workflow.version}</Typography>
                        </Stack>
                      </TableCell>
                      <TableCell>
                        {runs.isPending ? (
                          t('workflows.loading')
                        ) : runs.isError ? (
                          t('workflows.notAvailable')
                        ) : summary && summary.deadLetters > 0 ? (
                          <Chip
                            color="error"
                            label={t('workflows.deadLetters', {
                              count: summary.deadLetters,
                            })}
                            size="small"
                          />
                        ) : summary ? (
                          <Stack
                            spacing={0.25}
                            sx={{ alignItems: 'flex-start' }}
                          >
                            <RunStatusChip status={summary.latest.status} />
                            <Typography
                              color="text.secondary"
                              variant="caption"
                            >
                              <Timestamp
                                fallback={t('workflows.notAvailable')}
                                value={summary.latest.created_at}
                              />
                            </Typography>
                          </Stack>
                        ) : (
                          t('workflows.noRuns')
                        )}
                      </TableCell>
                      <TableCell align="right">
                        <RouterButton
                          params={{ workflowId: workflow.id }}
                          to={workflowRoutes.detail}
                          size="small"
                        >
                          {t('workflows.inspect')}
                        </RouterButton>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </Box>
          {families.length === 0 && (
            <EmptyState icon={WorkflowIcon} title={t('workflows.empty')} />
          )}
        </Paper>
      )}
    </PageContainer>
  );
};

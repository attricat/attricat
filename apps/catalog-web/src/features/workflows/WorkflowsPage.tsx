import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
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
import { type ComponentType } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { listWorkflowRuns, listWorkflows, type Workflow } from './api';
import { workflowQueryKeys } from './queryKeys';
import {
  workflowCapabilities,
  workflowRoutes,
  workflowRunStatus,
  workflowStatus,
} from './constants';
import { Timestamp } from '../../time/Timestamp';
import { WorkflowIcon } from '../../components/systemIcons';

const WorkflowDetailLink = Link as unknown as ComponentType<{
  params: { workflowId: string };
  to: typeof workflowRoutes.detail;
}>;

export const WorkflowsPage = () => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canRead =
    session.data?.capabilities?.[workflowCapabilities.read] === true;
  const canManage =
    session.data?.capabilities?.[workflowCapabilities.manage] === true;
  const workflows = useQuery({
    queryKey: workflowQueryKeys.all(),
    queryFn: listWorkflows,
    enabled: canRead,
  });
  const runs = useQuery({
    queryKey: workflowQueryKeys.runs(),
    queryFn: listWorkflowRuns,
    enabled: canRead,
  });
  const families = Object.values(
    (workflows.data ?? []).reduce<Record<string, Workflow>>(
      (result, workflow) => {
        if (
          !result[workflow.id] ||
          result[workflow.id].version < workflow.version
        )
          result[workflow.id] = workflow;
        return result;
      },
      {},
    ),
  );
  const runSummaries = new Map<
    string,
    {
      deadLetters: number;
      latest: Awaited<ReturnType<typeof listWorkflowRuns>>[number];
    }
  >();
  for (const run of runs.data ?? []) {
    const summary = runSummaries.get(run.workflow_id) ?? {
      deadLetters: 0,
      latest: run,
    };
    if (run.status === workflowRunStatus.deadLetter) summary.deadLetters += 1;
    if (run.created_at > summary.latest.created_at) summary.latest = run;
    runSummaries.set(run.workflow_id, summary);
  }

  if (session.isPending)
    return (
      <PageContainer>
        <Typography>{t('workflows.loading')}</Typography>
      </PageContainer>
    );
  if (!canRead)
    return (
      <PageContainer>
        <Alert severity="error">{t('workflows.notAuthorizedRead')}</Alert>
      </PageContainer>
    );

  return (
    <PageContainer>
      <PageHeader
        actions={
          canManage ? (
            <Button
              component={Link}
              to={workflowRoutes.create}
              variant="contained"
            >
              {t('workflows.newWorkflow')}
            </Button>
          ) : undefined
        }
        description={t('workflows.description')}
        icon={WorkflowIcon}
        title={t('workflows.title')}
      />
      {workflows.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {workflows.error.message}
        </Alert>
      )}
      {runs.isError && (
        <Alert severity="warning" sx={{ mt: 3 }}>
          {t('workflows.runsUnavailable', { message: runs.error.message })}
        </Alert>
      )}
      {workflows.isPending && (
        <Typography sx={{ mt: 3 }}>{t('workflows.loading')}</Typography>
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
                  <TableCell />
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
                          <Chip
                            color={
                              workflow.status === workflowStatus.published
                                ? 'success'
                                : 'warning'
                            }
                            label={t(`workflows.statuses.${workflow.status}`)}
                            size="small"
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
                          <Stack spacing={0.25}>
                            <Chip
                              label={t(
                                `workflows.statuses.${summary.latest.status}`,
                              )}
                              size="small"
                            />
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
                        <Button
                          component={WorkflowDetailLink}
                          params={{ workflowId: workflow.id }}
                          to={workflowRoutes.detail}
                          size="small"
                        >
                          {t('workflows.inspect')}
                        </Button>
                      </TableCell>
                    </TableRow>
                  );
                })}
              </TableBody>
            </Table>
          </Box>
          {families.length === 0 && (
            <Typography sx={{ p: 2 }}>{t('workflows.empty')}</Typography>
          )}
        </Paper>
      )}
    </PageContainer>
  );
};

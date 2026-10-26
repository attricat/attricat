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
import { PlusIcon } from 'lucide-react';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useCan } from '../auth/useCan';
import { listWorkflowRuns, listWorkflows, type Workflow } from './api';
import { workflowQueryKeys } from './queryKeys';
import { workflowRoutes, workflowRunStatus } from './constants';
import { RevisionStatusChip } from '../../components/RevisionStatusChip';
import { Timestamp } from '../../time/Timestamp';
import { EmptyState } from '../../components/EmptyState';
import { WorkflowIcon } from '../../components/systemIcons';
import { errorMessage } from '../../api/errors';
import { sessionOptions } from '../auth/queryOptions';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

const WorkflowDetailLink = Link as unknown as ComponentType<{
  params: { workflowId: string };
  to: typeof workflowRoutes.detail;
}>;

export const WorkflowsPage = () => {
  const { t } = useTranslation();
  const session = useQuery(sessionOptions());
  const canRead = useCan(capabilities.workflowsRead);
  const canManage = useCan(capabilities.workflowsManage);
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
              startIcon={<PlusIcon />}
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
      <ApiErrorAlert error={workflows.error} sx={{ mt: 3 }} />
      {runs.isError && (
        <Alert severity="warning" sx={{ mt: 3 }}>
          {t('workflows.runsUnavailable', {
            message: errorMessage(runs.error),
          })}
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
            <EmptyState icon={WorkflowIcon} title={t('workflows.empty')} />
          )}
        </Paper>
      )}
    </PageContainer>
  );
};

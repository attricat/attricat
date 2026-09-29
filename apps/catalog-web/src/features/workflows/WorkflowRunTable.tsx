import { useMutation, useQueryClient } from '@tanstack/react-query';
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
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import { replayWorkflowRun } from './api';
import { runTableEmptyColSpan, workflowRunStatus } from './constants';
import { formatWorkflowDateTime } from './dateTime';
import { workflowQueryKeys } from './queryKeys';
import type { WorkflowRun } from './schemas';

const runStatusColor = (status: WorkflowRun['status']) => {
  if (status === workflowRunStatus.deadLetter) return 'error';
  if (status === workflowRunStatus.completed) return 'success';
  return 'default';
};

export const WorkflowRunTable = ({
  canManage,
  locale,
  runs,
}: {
  canManage: boolean;
  locale: string;
  runs: WorkflowRun[];
}) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const replay = useMutation({
    mutationFn: (id: string) => replayWorkflowRun(id),
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: workflowQueryKeys.runs() }),
  });
  return (
    <Paper component="section" sx={{ mt: 3 }}>
      <Box sx={{ overflowX: 'auto' }}>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('workflows.status')}</TableCell>
              <TableCell>{t('workflows.source')}</TableCell>
              <TableCell>{t('workflows.attempts')}</TableCell>
              <TableCell>{t('workflows.created')}</TableCell>
              <TableCell>{t('workflows.outcomeEvidence')}</TableCell>
              <TableCell />
            </TableRow>
          </TableHead>
          <TableBody>
            {runs.map((run) => (
              <TableRow key={run.id}>
                <TableCell>
                  <Chip
                    color={runStatusColor(run.status)}
                    label={t(`workflows.statuses.${run.status}`)}
                    size="small"
                  />
                </TableCell>
                <TableCell>{run.source}</TableCell>
                <TableCell>{run.attempts}</TableCell>
                <TableCell>
                  {formatWorkflowDateTime(
                    run.completed_at ?? run.failed_at ?? run.created_at,
                    locale,
                    t('workflows.notAvailable'),
                  )}
                </TableCell>
                <TableCell>
                  {run.last_error ??
                    (run.status === workflowRunStatus.completed
                      ? t('workflows.completedEvidence')
                      : t('workflows.noOutcomeEvidence'))}
                </TableCell>
                <TableCell>
                  {canManage && run.status === workflowRunStatus.deadLetter && (
                    <Button
                      aria-label={t('workflows.replayRun', { id: run.id })}
                      disabled={replay.isPending}
                      onClick={() => replay.mutate(run.id)}
                      size="small"
                    >
                      {t('workflows.replay')}
                    </Button>
                  )}
                </TableCell>
              </TableRow>
            ))}
            {runs.length === 0 && (
              <TableRow>
                <TableCell colSpan={runTableEmptyColSpan}>
                  {t('workflows.noRuns')}
                </TableCell>
              </TableRow>
            )}
          </TableBody>
        </Table>
      </Box>
      {replay.isError && (
        <Alert severity="error" sx={{ m: 2 }}>
          {replay.error.message}
        </Alert>
      )}
    </Paper>
  );
};

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Chip,
  MenuItem,
  Paper,
  Stack,
  Tab,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/query-keys';
import {
  disableWorkflow,
  enableWorkflowRevision,
  listWorkflowRevisions,
  listWorkflowRuns,
  publishWorkflowRevision,
} from './api';
import { workflowQueryKeys } from './query-keys';

const dateTime = (value: string | null, locale: string, fallback: string) =>
  value
    ? new Intl.DateTimeFormat(locale, {
        dateStyle: 'medium',
        timeStyle: 'short',
      }).format(new Date(value))
    : fallback;

export const WorkflowDetailPage = ({ workflowId }: { workflowId: string }) => {
  const { i18n, t } = useTranslation();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState(0);
  const [leftVersion, setLeftVersion] = useState<number>();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canRead = session.data?.capabilities?.workflows_read === true;
  const canManage = session.data?.capabilities?.workflows_manage === true;
  const revisions = useQuery({
    queryKey: workflowQueryKeys.revisions(workflowId),
    queryFn: () => listWorkflowRevisions(workflowId),
    enabled: canRead,
  });
  const runs = useQuery({
    queryKey: workflowQueryKeys.runs(),
    queryFn: listWorkflowRuns,
    enabled: canRead,
  });
  const refresh = async () => {
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: workflowQueryKeys.all() }),
      queryClient.invalidateQueries({ queryKey: workflowQueryKeys.runs() }),
    ]);
  };
  const publish = useMutation({
    mutationFn: (version: number) =>
      publishWorkflowRevision(workflowId, version),
    onSuccess: refresh,
  });
  const enable = useMutation({
    mutationFn: (version: number) =>
      enableWorkflowRevision(workflowId, version),
    onSuccess: refresh,
  });
  const disable = useMutation({
    mutationFn: () => disableWorkflow(workflowId),
    onSuccess: refresh,
  });
  const current = revisions.data?.[0];
  const compared = revisions.data?.find(
    (revision) =>
      revision.version ===
      (leftVersion ?? revisions.data?.[1]?.version ?? current?.version),
  );
  const workflowRuns = (runs.data ?? []).filter(
    (run) => run.workflow_id === workflowId,
  );
  const mutationError = publish.error ?? enable.error ?? disable.error;
  const locale = i18n.resolvedLanguage ?? i18n.language;

  if (session.isPending || revisions.isPending)
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
  if (revisions.isError)
    return (
      <PageContainer>
        <Alert severity="error">{revisions.error.message}</Alert>
      </PageContainer>
    );
  if (!current)
    return (
      <PageContainer>
        <Alert severity="info">{t('workflows.empty')}</Alert>
      </PageContainer>
    );

  return (
    <PageContainer>
      <PageHeader
        actions={
          canManage ? (
            <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap' }}>
              <Link
                params={{ workflowId, version: String(current.version) }}
                to="/manage/workflows/$workflowId/revisions/$version/new"
              >
                <Button variant="outlined">{t('workflows.newRevision')}</Button>
              </Link>
              {current.status === 'draft' && (
                <Button
                  disabled={publish.isPending}
                  onClick={() => publish.mutate(current.version)}
                  variant="contained"
                >
                  {t('workflows.publish')}
                </Button>
              )}
              {current.status === 'published' &&
                current.enabled_version !== current.version && (
                  <Button
                    disabled={enable.isPending}
                    onClick={() => enable.mutate(current.version)}
                    variant="contained"
                  >
                    {t('workflows.enable')}
                  </Button>
                )}
              {current.enabled_version !== null && (
                <Button
                  color="warning"
                  disabled={disable.isPending}
                  onClick={() => disable.mutate()}
                >
                  {t('workflows.disable')}
                </Button>
              )}
            </Stack>
          ) : undefined
        }
        eyebrow={t('workflows.workflow')}
        title={current.name}
      />
      <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', mt: 1 }}>
        <Chip label={current.code} variant="outlined" />
        <Chip
          color={current.status === 'published' ? 'success' : 'warning'}
          label={current.status}
        />
        <Chip
          color={current.enabled_version === null ? 'default' : 'success'}
          label={
            current.enabled_version === null
              ? t('workflows.disabled')
              : t('workflows.enabledRevision', {
                  version: current.enabled_version,
                })
          }
        />
      </Stack>
      {!canManage && (
        <Alert severity="info" sx={{ mt: 2 }}>
          {t('workflows.readOnly')}
        </Alert>
      )}
      {mutationError && (
        <Alert severity="error" sx={{ mt: 2 }}>
          {mutationError.message}
        </Alert>
      )}
      {runs.isError && (
        <Alert severity="warning" sx={{ mt: 2 }}>
          {t('workflows.runsUnavailable', { message: runs.error.message })}
        </Alert>
      )}
      <Tabs
        allowScrollButtonsMobile
        onChange={(_, value: number) => setTab(value)}
        scrollButtons="auto"
        sx={{ mt: 3 }}
        value={tab}
        variant="scrollable"
      >
        <Tab label={t('workflows.revisions')} />
        <Tab label={t('workflows.source')} />
        <Tab label={t('workflows.compare')} />
        <Tab label={t('workflows.runDiagnostics')} />
      </Tabs>
      {tab === 0 && <RevisionTable revisions={revisions.data} />}
      {tab === 1 && (
        <Source definition={current.definition} title={t('workflows.source')} />
      )}
      {tab === 2 && (
        <Paper component="section" sx={{ mt: 3, p: 2 }}>
          <TextField
            label={t('workflows.compareRevision')}
            onChange={(event) => setLeftVersion(Number(event.target.value))}
            select
            sx={{ minWidth: 220 }}
            value={compared?.version ?? ''}
          >
            {revisions.data.map((revision) => (
              <MenuItem key={revision.version} value={revision.version}>
                v{revision.version} ({revision.status})
              </MenuItem>
            ))}
          </TextField>
          <Box
            sx={{
              display: 'grid',
              gap: 2,
              gridTemplateColumns: { md: '1fr 1fr' },
              mt: 2,
            }}
          >
            <Source
              definition={compared?.definition ?? ''}
              title={t('workflows.compareRevision')}
            />
            <Source
              definition={current.definition}
              title={t('workflows.currentRevision')}
            />
          </Box>
        </Paper>
      )}
      {tab === 3 && (
        <RunTable canManage={canManage} locale={locale} runs={workflowRuns} />
      )}
    </PageContainer>
  );
};

const RevisionTable = ({
  revisions,
}: {
  revisions: Awaited<ReturnType<typeof listWorkflowRevisions>>;
}) => {
  const { i18n, t } = useTranslation();
  const locale = i18n.resolvedLanguage ?? i18n.language;
  return (
    <Paper component="section" sx={{ mt: 3 }}>
      <Box sx={{ overflowX: 'auto' }}>
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>{t('workflows.currentRevision')}</TableCell>
              <TableCell>{t('workflows.status')}</TableCell>
              <TableCell>{t('workflows.created')}</TableCell>
              <TableCell>{t('workflows.published')}</TableCell>
              <TableCell>{t('workflows.definitionHash')}</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {revisions.map((revision) => (
              <TableRow key={revision.version}>
                <TableCell>v{revision.version}</TableCell>
                <TableCell>
                  <Chip
                    color={
                      revision.status === 'published' ? 'success' : 'warning'
                    }
                    label={revision.status}
                    size="small"
                  />
                </TableCell>
                <TableCell>
                  {dateTime(
                    revision.created_at,
                    locale,
                    t('workflows.notAvailable'),
                  )}
                </TableCell>
                <TableCell>
                  {dateTime(
                    revision.published_at,
                    locale,
                    t('workflows.notAvailable'),
                  )}
                </TableCell>
                <TableCell sx={{ fontFamily: 'monospace' }}>
                  {revision.definition_hash}
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Box>
    </Paper>
  );
};

const Source = ({
  definition,
  title,
}: {
  definition: string;
  title?: string;
}) => (
  <Paper component="section" sx={{ mt: title ? 0 : 3, p: 2 }}>
    <Typography component="h2" variant="h6">
      {title}
    </Typography>
    <Box
      aria-label={title}
      component="pre"
      sx={{
        fontFamily: 'monospace',
        m: 0,
        mt: title ? 1 : 0,
        overflow: 'auto',
        whiteSpace: 'pre-wrap',
      }}
    >
      {definition}
    </Box>
  </Paper>
);

const RunTable = ({
  canManage,
  locale,
  runs,
}: {
  canManage: boolean;
  locale: string;
  runs: Awaited<ReturnType<typeof listWorkflowRuns>>;
}) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const replay = useMutation({
    mutationFn: (id: string) =>
      import('./api').then(({ replayWorkflowRun }) => replayWorkflowRun(id)),
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
                    color={
                      run.status === 'dead_letter'
                        ? 'error'
                        : run.status === 'completed'
                          ? 'success'
                          : 'default'
                    }
                    label={run.status}
                    size="small"
                  />
                </TableCell>
                <TableCell>{run.attempts}</TableCell>
                <TableCell>
                  {dateTime(
                    run.completed_at ?? run.failed_at ?? run.created_at,
                    locale,
                    t('workflows.notAvailable'),
                  )}
                </TableCell>
                <TableCell>
                  {run.last_error ??
                    (run.status === 'completed'
                      ? t('workflows.completedEvidence')
                      : t('workflows.noOutcomeEvidence'))}
                </TableCell>
                <TableCell>
                  {canManage && run.status === 'dead_letter' && (
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
                <TableCell colSpan={5}>{t('workflows.noRuns')}</TableCell>
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

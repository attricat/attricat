import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Box, Chip, Stack, Tab, Tabs, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import {
  disableWorkflow,
  enableWorkflowRevision,
  listWorkflowRevisions,
  listWorkflowRuns,
  publishWorkflowRevision,
  runWorkflowNow,
} from './api';
import {
  workflowCapabilities,
  workflowDetailTab,
  type WorkflowDetailTab,
  workflowStatus,
} from './constants';
import { workflowQueryKeys } from './queryKeys';
import { WorkflowActions } from './WorkflowActions';
import { WorkflowComparePanel } from './WorkflowComparePanel';
import { WorkflowRevisionTable } from './WorkflowRevisionTable';
import { WorkflowRunTable } from './WorkflowRunTable';
import { WorkflowSourcePanel } from './WorkflowSourcePanel';
import { WorkflowIcon } from '../../components/systemIcons';

export const WorkflowDetailPage = ({ workflowId }: { workflowId: string }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<WorkflowDetailTab>(
    workflowDetailTab.revisions,
  );
  const [leftVersion, setLeftVersion] = useState<number>();
  const tabId = useTabAccessibility();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canRead =
    session.data?.capabilities?.[workflowCapabilities.read] === true;
  const canManage =
    session.data?.capabilities?.[workflowCapabilities.manage] === true;
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
      queryClient.invalidateQueries({
        queryKey: workflowQueryKeys.revisions(workflowId),
      }),
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
  const runNow = useMutation({
    mutationFn: (entityId: string) =>
      runWorkflowNow(workflowId, entityId, crypto.randomUUID()),
    onSuccess: refresh,
  });
  const disable = useMutation({
    mutationFn: () => disableWorkflow(workflowId),
    onSuccess: refresh,
  });
  const current = revisions.data?.[0];
  useResourcePageTitle(current?.name, t('navigation.workflows'));
  const compared = revisions.data?.find(
    (revision) =>
      revision.version ===
      (leftVersion ?? revisions.data?.[1]?.version ?? current?.version),
  );
  const workflowRuns = (runs.data ?? []).filter(
    (run) => run.workflow_id === workflowId,
  );
  const mutationError =
    publish.error ?? enable.error ?? runNow.error ?? disable.error;

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
            <WorkflowActions
              current={current}
              disable={disable}
              enable={enable}
              publish={publish}
              runNow={runNow}
              workflowId={workflowId}
            />
          ) : undefined
        }
        eyebrow={t('workflows.workflow')}
        icon={WorkflowIcon}
        title={current.name}
      />
      <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', mt: 1 }}>
        <Chip label={current.code} variant="outlined" />
        <Chip
          color={
            current.status === workflowStatus.published ? 'success' : 'warning'
          }
          label={t(`workflows.statuses.${current.status}`)}
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
        onChange={(_, value: WorkflowDetailTab) => setTab(value)}
        scrollButtons="auto"
        sx={{ mt: 3 }}
        value={tab}
        variant="scrollable"
      >
        <Tab
          {...tabId.tab(workflowDetailTab.revisions)}
          label={t('workflows.revisions')}
          value={workflowDetailTab.revisions}
        />
        <Tab
          {...tabId.tab(workflowDetailTab.source)}
          label={t('workflows.source')}
          value={workflowDetailTab.source}
        />
        <Tab
          {...tabId.tab(workflowDetailTab.compare)}
          label={t('workflows.compare')}
          value={workflowDetailTab.compare}
        />
        <Tab
          {...tabId.tab(workflowDetailTab.runDiagnostics)}
          label={t('workflows.runDiagnostics')}
          value={workflowDetailTab.runDiagnostics}
        />
      </Tabs>
      {tab === workflowDetailTab.revisions && (
        <Box {...tabId.panel(workflowDetailTab.revisions)}>
          <WorkflowRevisionTable revisions={revisions.data} />
        </Box>
      )}
      {tab === workflowDetailTab.source && (
        <Box {...tabId.panel(workflowDetailTab.source)}>
          <WorkflowSourcePanel
            definition={current.definition}
            title={t('workflows.source')}
          />
        </Box>
      )}
      {tab === workflowDetailTab.compare && (
        <Box {...tabId.panel(workflowDetailTab.compare)}>
          <WorkflowComparePanel
            compared={compared}
            current={current}
            onSelectVersion={setLeftVersion}
            revisions={revisions.data}
          />
        </Box>
      )}
      {tab === workflowDetailTab.runDiagnostics && (
        <Box {...tabId.panel(workflowDetailTab.runDiagnostics)}>
          <WorkflowRunTable canManage={canManage} runs={workflowRuns} />
        </Box>
      )}
    </PageContainer>
  );
};

import { RevisionStatusChip } from '../../components/RevisionStatusChip';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Alert, Box, Chip, Stack, Tab, Tabs } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  CodeIcon,
  GitCompareArrowsIcon,
  HistoryIcon,
  StethoscopeIcon,
} from 'lucide-react';
import { compactIconSize } from '../../components/iconSizes';
import { useResourcePageTitle } from '../../app/useResourcePageTitle';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useTabAccessibility } from '../../components/useTabAccessibility';
import { useCan } from '../auth/useCan';
import {
  disableWorkflow,
  enableWorkflowRevision,
  publishWorkflowRevision,
  runWorkflowNow,
} from './api';
import { workflowDetailTab, type WorkflowDetailTab } from './constants';
import { workflowQueryKeys } from './queryKeys';
import { WorkflowActions } from './WorkflowActions';
import { WorkflowComparePanel } from './WorkflowComparePanel';
import { WorkflowRevisionTable } from './WorkflowRevisionTable';
import { WorkflowRunTable } from './WorkflowRunTable';
import { WorkflowSourcePanel } from './WorkflowSourcePanel';
import { WorkflowIcon } from '../../components/systemIcons';
import { capabilities } from '../auth/capabilities';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { randomUuid } from '../../components/randomUuid';
import {
  workflowRevisionListOptions,
  workflowRunListOptions,
} from './queryOptions';
import { RunsUnavailableAlert } from './RunsUnavailableAlert';
import { LoadingState } from '../../components/LoadingState';

export const WorkflowDetailPage = ({ workflowId }: { workflowId: string }) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [tab, setTab] = useState<WorkflowDetailTab>(
    workflowDetailTab.revisions,
  );
  const [leftVersion, setLeftVersion] = useState<number>();
  const tabId = useTabAccessibility();
  const canManage = useCan(capabilities.workflowsManage);
  const revisions = useQuery(workflowRevisionListOptions(workflowId));
  const runs = useQuery(workflowRunListOptions());
  // Revisions and runs are both under the workflows root key.
  const refresh = () =>
    queryClient.invalidateQueries({ queryKey: workflowQueryKeys.all() });
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
    mutationFn: (recordId: string) =>
      runWorkflowNow(workflowId, recordId, randomUuid()),
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

  if (revisions.isPending)
    return (
      <PageContainer>
        <LoadingState>{t('workflows.loading')}</LoadingState>
      </PageContainer>
    );
  if (revisions.isError)
    return (
      <PageContainer>
        <ApiErrorAlert error={revisions.error} />
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
        <RevisionStatusChip status={current.status} />
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
      <ApiErrorAlert error={mutationError} sx={{ mt: 2 }} />
      <RunsUnavailableAlert error={runs.error} sx={{ mt: 2 }} />
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
          icon={<HistoryIcon aria-hidden size={compactIconSize} />}
          label={t('workflows.revisions')}
          value={workflowDetailTab.revisions}
        />
        <Tab
          {...tabId.tab(workflowDetailTab.source)}
          icon={<CodeIcon aria-hidden size={compactIconSize} />}
          label={t('workflows.source')}
          value={workflowDetailTab.source}
        />
        <Tab
          {...tabId.tab(workflowDetailTab.compare)}
          icon={<GitCompareArrowsIcon aria-hidden size={compactIconSize} />}
          label={t('workflows.compare')}
          value={workflowDetailTab.compare}
        />
        <Tab
          {...tabId.tab(workflowDetailTab.runDiagnostics)}
          icon={<StethoscopeIcon aria-hidden size={compactIconSize} />}
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

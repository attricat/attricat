import { useQuery } from '@tanstack/react-query';
import { Alert, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { getWorkflowRevision } from './api';
import { workflowCapabilities } from './constants';
import { workflowQueryKeys } from './queryKeys';
import { WorkflowDefinitionForm } from './WorkflowDefinitionForm';
import { WorkflowIcon } from '../../components/systemIcons';

export const WorkflowEditorPage = ({
  workflowId,
  sourceVersion,
}: {
  workflowId?: string;
  sourceVersion?: number;
}) => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canManage =
    session.data?.capabilities?.[workflowCapabilities.manage] === true;
  const source = useQuery({
    queryKey: workflowQueryKeys.revision(workflowId ?? '', sourceVersion ?? 0),
    queryFn: () => getWorkflowRevision(workflowId!, sourceVersion!),
    enabled: Boolean(workflowId && sourceVersion && canManage),
  });

  if (session.isPending || (workflowId && source.isPending))
    return (
      <PageContainer>
        <Typography>{t('workflows.loading')}</Typography>
      </PageContainer>
    );
  if (!canManage)
    return (
      <PageContainer>
        <Alert severity="error">{t('workflows.notAuthorizedManage')}</Alert>
      </PageContainer>
    );

  return (
    <PageContainer>
      <PageHeader
        description={
          workflowId
            ? t('workflows.newRevisionDescription', { version: sourceVersion })
            : t('workflows.newDescription')
        }
        icon={WorkflowIcon}
        title={
          workflowId ? t('workflows.newRevision') : t('workflows.newWorkflow')
        }
      />
      {source.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {source.error.message}
        </Alert>
      )}
      <WorkflowDefinitionForm
        key={`${workflowId}:${sourceVersion}`}
        readOnly={source.isError}
        sourceDefinition={source.data?.definition}
        sourceVersion={sourceVersion}
        workflowId={workflowId}
      />
    </PageContainer>
  );
};

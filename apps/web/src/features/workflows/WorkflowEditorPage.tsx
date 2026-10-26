import { useQuery } from '@tanstack/react-query';
import { Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { getWorkflowRevision } from './api';
import { workflowQueryKeys } from './queryKeys';
import { WorkflowDefinitionForm } from './WorkflowDefinitionForm';
import { WorkflowIcon } from '../../components/systemIcons';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';

export const WorkflowEditorPage = ({
  workflowId,
  sourceVersion,
}: {
  workflowId?: string;
  sourceVersion?: number;
}) => {
  const { t } = useTranslation();
  const source = useQuery({
    queryKey: workflowQueryKeys.revision(workflowId ?? '', sourceVersion ?? 0),
    queryFn: () => getWorkflowRevision(workflowId!, sourceVersion!),
    enabled: Boolean(workflowId && sourceVersion),
  });

  if (workflowId && source.isPending)
    return (
      <PageContainer>
        <Typography>{t('workflows.loading')}</Typography>
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
      <ApiErrorAlert error={source.error} sx={{ mt: 3 }} />
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

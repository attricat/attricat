import { useQuery } from '@tanstack/react-query';
import { Alert, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { useCan } from '../auth/useCan';
import { getWorkflowRevision } from './api';
import { workflowCapabilities } from './constants';
import { workflowQueryKeys } from './queryKeys';
import { WorkflowDefinitionForm } from './WorkflowDefinitionForm';
import { WorkflowIcon } from '../../components/systemIcons';
import { errorMessage } from '../../api/errors';
import { sessionOptions } from '../auth/queryOptions';

export const WorkflowEditorPage = ({
  workflowId,
  sourceVersion,
}: {
  workflowId?: string;
  sourceVersion?: number;
}) => {
  const { t } = useTranslation();
  const session = useQuery(sessionOptions());
  const canManage = useCan(workflowCapabilities.manage);
  const source = useQuery({
    queryKey: workflowQueryKeys.revision(workflowId ?? '', sourceVersion ?? 0),
    queryFn: () => getWorkflowRevision(workflowId!, sourceVersion!),
    enabled: Boolean(workflowId && sourceVersion && canManage),
  });

  if (session.isPending)
    return (
      <PageContainer>
        <Typography>{t('workflows.loading')}</Typography>
      </PageContainer>
    );
  // The source query stays pending while disabled, so check access first.
  if (!canManage)
    return (
      <PageContainer>
        <Alert severity="error">{t('workflows.notAuthorizedManage')}</Alert>
      </PageContainer>
    );
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
      {source.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {errorMessage(source.error)}
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

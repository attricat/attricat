import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Paper, Stack, Typography } from '@mui/material';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { currentSession } from '../auth/api';
import { TomlEditor } from '../../components/TomlEditor';
import { authQueryKeys } from '../auth/queryKeys';
import {
  createWorkflow,
  createWorkflowRevision,
  getWorkflowRevision,
  validateWorkflow,
} from './api';
import {
  editorHeight,
  starterWorkflowDefinition,
  workflowCapabilities,
  workflowRoutes,
} from './constants';
import { workflowQueryKeys } from './queryKeys';

export const WorkflowEditorPage = ({
  workflowId,
  sourceVersion,
}: {
  workflowId?: string;
  sourceVersion?: number;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
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
  const validate = useMutation({ mutationFn: validateWorkflow });
  const hydratedSource = useRef<string | undefined>(undefined);
  const save = useMutation({
    mutationFn: (definition: string) =>
      workflowId
        ? createWorkflowRevision(workflowId, definition)
        : createWorkflow(definition),
    onSuccess: async (workflow) => {
      await queryClient.invalidateQueries({
        queryKey: workflowQueryKeys.all(),
      });
      await navigate({
        params: { workflowId: workflow.id },
        to: workflowRoutes.detail,
      });
    },
  });
  const form = useForm({
    defaultValues: { definition: workflowId ? '' : starterWorkflowDefinition },
    onSubmit: ({ value }) => {
      if (!save.isPending) save.mutate(value.definition);
    },
  });
  useEffect(() => {
    const sourceKey = `${workflowId}:${sourceVersion}`;
    if (source.data && hydratedSource.current !== sourceKey) {
      form.setFieldValue('definition', source.data.definition);
      hydratedSource.current = sourceKey;
    }
  }, [form, source.data, sourceVersion, workflowId]);

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
        title={
          workflowId ? t('workflows.newRevision') : t('workflows.newWorkflow')
        }
      />
      {source.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {source.error.message}
        </Alert>
      )}
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
        sx={{ mt: 3, p: 3 }}
      >
        <Stack spacing={2}>
          <Typography color="text.secondary">
            {t('workflows.validationHelp')}
          </Typography>
          <form.Field name="definition">
            {(field) => (
              <TomlEditor
                ariaLabel={t('workflows.tomlDefinition')}
                height={editorHeight}
                onChange={(value) => {
                  if (save.isPending) return;
                  validate.reset();
                  field.handleChange(value ?? '');
                }}
                readOnly={source.isError || save.isPending}
                value={field.state.value}
              />
            )}
          </form.Field>
          {(validate.error || save.error) && (
            <Alert severity="error">
              {(validate.error ?? save.error)?.message}
            </Alert>
          )}
          {validate.data && (
            <Alert severity="success">
              {t('workflows.validationSucceeded', {
                actions: validate.data.actions.length,
                name: validate.data.name,
                triggers: validate.data.triggers.length,
              })}
            </Alert>
          )}
          <Stack direction="row" spacing={1}>
            <form.Subscribe selector={(state) => state.values.definition}>
              {(definition) => (
                <Button
                  disabled={
                    validate.isPending ||
                    save.isPending ||
                    source.isError ||
                    !definition.trim()
                  }
                  onClick={() => validate.mutate(definition)}
                  variant="outlined"
                >
                  {t('workflows.validate')}
                </Button>
              )}
            </form.Subscribe>
            <Button
              disabled={save.isPending || source.isError}
              type="submit"
              variant="contained"
            >
              {save.isPending
                ? t('workflows.saving')
                : t('workflows.saveDraft')}
            </Button>
          </Stack>
        </Stack>
      </Paper>
    </PageContainer>
  );
};

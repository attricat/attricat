import { useForm, useStore } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Paper, Stack, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { TomlEditor } from '../../components/TomlEditor';
import { draftEditors } from '../drafts/constants';
import { DraftRestoreDialog } from '../drafts/DraftRestoreDialog';
import { definitionDraftSchema } from '../drafts/schemas';
import { useEditorDraft } from '../drafts/useEditorDraft';
import {
  createWorkflow,
  createWorkflowRevision,
  validateWorkflow,
} from './api';
import {
  editorHeight,
  starterWorkflowDefinition,
  workflowRoutes,
} from './constants';
import { workflowQueryKeys } from './queryKeys';

/** Edits a workflow definition starting from its loaded source revision. */
export const WorkflowDefinitionForm = ({
  readOnly,
  sourceDefinition,
  sourceVersion,
  workflowId,
}: {
  readOnly: boolean;
  sourceDefinition?: string;
  sourceVersion?: number;
  workflowId?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const initialDefinition = workflowId
    ? (sourceDefinition ?? '')
    : starterWorkflowDefinition;
  const validate = useMutation({ mutationFn: validateWorkflow });
  const save = useMutation({
    mutationFn: (definition: string) =>
      workflowId
        ? createWorkflowRevision(workflowId, definition)
        : createWorkflow(definition),
    onSuccess: async (workflow) => {
      draft.clear();
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
    defaultValues: { definition: initialDefinition },
    onSubmit: ({ value }) => {
      if (!save.isPending) save.mutate(value.definition);
    },
  });
  const definition = useStore(form.store, (state) => state.values.definition);
  const draft = useEditorDraft({
    dirty: definition !== initialDefinition,
    editor: workflowId
      ? draftEditors.workflowRevision
      : draftEditors.workflowCreate,
    ready: !workflowId || sourceDefinition !== undefined,
    resource: workflowId ? [workflowId, sourceVersion ?? 0] : [],
    schema: definitionDraftSchema,
    source: sourceDefinition ?? null,
    value: definition,
  });

  const restoreDraft = () => {
    const restored = draft.restore();
    if (restored === undefined) return;
    validate.reset();
    form.setFieldValue('definition', restored);
  };

  return (
    <Paper
      component="form"
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
      sx={{ mt: 3, p: 3 }}
    >
      <DraftRestoreDialog
        draft={draft.pending}
        onDiscard={draft.discard}
        onRestore={restoreDraft}
      />
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
              readOnly={readOnly || save.isPending}
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
          <Button
            disabled={
              validate.isPending ||
              save.isPending ||
              readOnly ||
              !definition.trim()
            }
            onClick={() => validate.mutate(definition)}
            variant="outlined"
          >
            {t('workflows.validate')}
          </Button>
          <Button
            disabled={save.isPending || readOnly}
            type="submit"
            variant="contained"
          >
            {save.isPending ? t('workflows.saving') : t('workflows.saveDraft')}
          </Button>
        </Stack>
      </Stack>
    </Paper>
  );
};

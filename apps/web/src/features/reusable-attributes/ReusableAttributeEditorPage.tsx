import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Chip, Stack, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  createReusableAttribute,
  createReusableAttributeRevision,
  listReusableAttributes,
  validateReusableAttribute,
} from './api';
import { definitionKinds } from '../definition-editor/constants';
import { DefinitionTemplateDialog } from '../definition-editor/DefinitionTemplateDialog';
import { DefinitionWorkspace } from '../definition-editor/DefinitionWorkspace';
import { UnsavedDefinitionChangesDialog } from '../definition-editor/UnsavedDefinitionChangesDialog';
import { useDefinitionCheck } from '../definition-editor/useDefinitionCheck';
import { draftEditors } from '../drafts/constants';
import { DraftRestoreDialog } from '../drafts/DraftRestoreDialog';
import { definitionDraftSchema } from '../drafts/schemas';
import { useEditorDraft } from '../drafts/useEditorDraft';
import { reusableAttributeQueryKeys } from './queryKeys';
import { latestReusableAttributeRevisions } from './latestRevisions';
import { ReusableAttributePreview } from './ReusableAttributePreview';
import { reusableAttributeTemplates } from './templates';
import { ReusableAttributeIcon } from '../../components/systemIcons';
import { lexiconText } from '../lexicon/lexicon';

type PendingUnsavedAction =
  { templateIndex: number; type: 'replace' } | { type: 'discard' };

export const ReusableAttributeEditorPage = ({
  definitionId,
}: {
  definitionId?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const attributes = useQuery({
    queryKey: reusableAttributeQueryKeys.definitions(true),
    queryFn: ({ signal }) => listReusableAttributes(true, signal),
  });
  const attribute = latestReusableAttributeRevisions(
    attributes.data ?? [],
  ).find((item) => item.definition_id === definitionId);
  const [templateIndex, setTemplateIndex] = useState(0);
  const [templateDialogOpen, setTemplateDialogOpen] = useState(!definitionId);
  const [pendingUnsavedAction, setPendingUnsavedAction] =
    useState<PendingUnsavedAction>();
  const [editedDefinition, setEditedDefinition] = useState<string | null>(null);
  const initialDefinition =
    attribute?.definition ??
    reusableAttributeTemplates[templateIndex].definition;
  const definition = editedDefinition ?? initialDefinition;
  const isDirty = definition !== initialDefinition;
  const draft = useEditorDraft({
    dirty: isDirty,
    editor: definitionId
      ? draftEditors.reusableAttributeRevision
      : draftEditors.reusableAttributeCreate,
    ready: !definitionId || Boolean(attribute),
    resource: definitionId ? [definitionId] : [],
    schema: definitionDraftSchema,
    source: attribute?.definition ?? null,
    value: definition,
  });
  const save = useMutation({
    mutationFn: (submittedDefinition: string) =>
      attribute
        ? createReusableAttributeRevision(attribute.definition_id, {
            definition: submittedDefinition,
          })
        : createReusableAttribute({ definition: submittedDefinition }),
    onSuccess: async () => {
      draft.clear();
      await queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.root(),
      });
      navigate({ to: '/manage/reusable-attributes' });
    },
  });
  const canSave = !save.isPending && (!definitionId || isDirty);
  const saveDefinition = () => {
    if (canSave) save.mutate(definition);
  };
  const check = useDefinitionCheck({
    definition,
    kind: definitionKinds.reusableAttribute,
    resourceId: attribute?.definition_id,
    validate: (value, id, signal) =>
      validateReusableAttribute({ definition: value }, id, signal),
  });

  const applyTemplate = (index: number) => {
    setTemplateIndex(index);
    setEditedDefinition(null);
    setTemplateDialogOpen(false);
  };

  const selectTemplate = (index: number) => {
    if (isDirty) {
      setPendingUnsavedAction({ templateIndex: index, type: 'replace' });
      return;
    }
    applyTemplate(index);
  };

  const leaveEditor = () => navigate({ to: '/manage/reusable-attributes' });

  const cancel = () => {
    if (isDirty) {
      setPendingUnsavedAction({ type: 'discard' });
      return;
    }
    leaveEditor();
  };

  const confirmUnsavedAction = () => {
    if (pendingUnsavedAction?.type === 'replace') {
      applyTemplate(pendingUnsavedAction.templateIndex);
    } else if (pendingUnsavedAction?.type === 'discard') {
      leaveEditor();
    }
    setPendingUnsavedAction(undefined);
  };

  if (definitionId && attributes.isPending) {
    return (
      <PageContainer>
        <Typography>{t('reusableAttributes.editor.loading')}</Typography>
      </PageContainer>
    );
  }

  if (definitionId && !attribute) {
    return (
      <PageContainer>
        <Alert severity="error">
          {t('reusableAttributes.editor.notFound')}
        </Alert>
      </PageContainer>
    );
  }

  return (
    <PageContainer>
      <PageHeader
        description={t('reusableAttributes.editor.description')}
        icon={ReusableAttributeIcon}
        title={
          attribute
            ? t('reusableAttributes.editor.editTitle', {
                name: lexiconText(attribute.name),
              })
            : t('reusableAttributes.editor.newTitle')
        }
        actions={
          <Stack direction="row" spacing={1}>
            {isDirty && (
              <Chip
                color="warning"
                label={t('definitionEditor.unsaved.chip')}
              />
            )}
            <Button disabled={save.isPending} onClick={cancel}>
              {t('reusableAttributes.editor.cancel')}
            </Button>
            <Button
              disabled={!canSave}
              onClick={saveDefinition}
              variant="contained"
            >
              {attribute
                ? t('reusableAttributes.editor.saveRevision')
                : t('reusableAttributes.editor.createAttribute')}
            </Button>
          </Stack>
        }
      />
      {save.error && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {save.error.message}
        </Alert>
      )}
      {!definitionId && (
        <DefinitionTemplateDialog
          onClose={() => setTemplateDialogOpen(false)}
          onSelect={selectTemplate}
          open={templateDialogOpen && !draft.pending}
          selectedIndex={templateIndex}
          templates={reusableAttributeTemplates}
        />
      )}
      <DraftRestoreDialog
        draft={draft.pending}
        onDiscard={draft.discard}
        onRestore={() => {
          const restored = draft.restore();
          if (restored === undefined) return;
          setEditedDefinition(restored);
          setTemplateDialogOpen(false);
        }}
      />
      <UnsavedDefinitionChangesDialog
        action={pendingUnsavedAction?.type}
        onCancel={() => setPendingUnsavedAction(undefined)}
        onConfirm={confirmUnsavedAction}
      />
      <DefinitionWorkspace
        check={check}
        kind={definitionKinds.reusableAttribute}
        onChange={setEditedDefinition}
        onSave={saveDefinition}
        preview={
          check.result && (
            <ReusableAttributePreview
              key={check.result.value_type}
              reusable={check.result}
            />
          )
        }
        readOnly={save.isPending}
        toolbar={
          !definitionId && (
            <Button
              disabled={save.isPending}
              onClick={() => setTemplateDialogOpen(true)}
            >
              {t('definitionEditor.templates.button')}
            </Button>
          )
        }
        value={definition}
      />
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {definitionId && !isDirty
          ? t('definitionEditor.makeChangeBeforeSaving')
          : t('definitionEditor.saveShortcut')}
      </Typography>
    </PageContainer>
  );
};

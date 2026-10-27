import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import {
  createReusableAttribute,
  createReusableAttributeRevision,
  validateReusableAttribute,
} from './api';
import { definitionKinds } from '../definition-editor/constants';
import { DefinitionWorkspace } from '../definition-editor/DefinitionWorkspace';
import { useDefinitionCheck } from '../definition-editor/useDefinitionCheck';
import { draftEditors } from '../drafts/constants';
import { reusableAttributeQueryKeys } from './queryKeys';
import { latestReusableAttributeRevisions } from './latestRevisions';
import { ReusableAttributePreview } from './ReusableAttributePreview';
import { reusableAttributeTemplates } from './templates';
import { ReusableAttributeIcon } from '../../components/systemIcons';
import { lexiconText } from '../lexicon/lexicon';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { useDefinitionEditor } from '../definition-editor/useDefinitionEditor';
import {
  DefinitionEditorActions,
  DefinitionSaveHint,
  DefinitionTemplatesButton,
} from '../definition-editor/DefinitionEditorControls';
import { reusableAttributeListOptions } from './queryOptions';

export const ReusableAttributeEditorPage = ({
  definitionId,
}: {
  definitionId?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const attributes = useQuery(reusableAttributeListOptions(true));
  const attribute = latestReusableAttributeRevisions(
    attributes.data ?? [],
  ).find((item) => item.definition_id === definitionId);
  const leaveEditor = () => navigate({ to: '/manage/reusable-attributes' });
  const editor = useDefinitionEditor({
    draftEditor: definitionId
      ? draftEditors.reusableAttributeRevision
      : draftEditors.reusableAttributeCreate,
    draftResource: definitionId ? [definitionId] : [],
    onLeave: leaveEditor,
    ready: !definitionId || Boolean(attribute),
    source: attribute?.definition,
    templates: reusableAttributeTemplates,
  });
  const { definition, isDirty } = editor;
  const save = useMutation({
    mutationFn: (submittedDefinition: string) =>
      attribute
        ? createReusableAttributeRevision(attribute.definition_id, {
            definition: submittedDefinition,
          })
        : createReusableAttribute({ definition: submittedDefinition }),
    onSuccess: async () => {
      editor.clearDraft();
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
        {attributes.isError ? (
          <ApiErrorAlert error={attributes.error} />
        ) : (
          <Alert severity="error">
            {t('reusableAttributes.editor.notFound')}
          </Alert>
        )}
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
          <DefinitionEditorActions
            canSave={canSave}
            isDirty={isDirty}
            isSaving={save.isPending}
            onCancel={editor.cancel}
            onSave={saveDefinition}
            saveLabel={
              attribute
                ? t('reusableAttributes.editor.saveRevision')
                : t('reusableAttributes.editor.createAttribute')
            }
          />
        }
      />
      <ApiErrorAlert error={save.error} sx={{ mt: 3 }} />
      {editor.dialogs}
      <DefinitionWorkspace
        check={check}
        kind={definitionKinds.reusableAttribute}
        onChange={editor.setDefinition}
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
          editor.creating && (
            <DefinitionTemplatesButton
              disabled={save.isPending}
              onClick={editor.openTemplates}
            />
          )
        }
        value={definition}
      />
      <DefinitionSaveHint unchanged={!editor.creating && !isDirty} />
    </PageContainer>
  );
};

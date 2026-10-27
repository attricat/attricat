import { blueprintRevisionOptions } from './queryOptions';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Typography } from '@mui/material';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { definitionKinds } from '../definition-editor/constants';
import { DefinitionWorkspace } from '../definition-editor/DefinitionWorkspace';
import { useDefinitionCheck } from '../definition-editor/useDefinitionCheck';
import { draftEditors } from '../drafts/constants';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import {
  createBlueprint,
  createBlueprintRevision,
  validateBlueprint,
} from './api';
import { blueprintTemplates } from './blueprintEditorUtils';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';
import { recordQueryKeys } from '../records/queryKeys';
import { blueprintQueryKeys } from './queryKeys';
import { BlueprintIcon } from '../../components/systemIcons';
import { errorMessage } from '../../api/errors';
import { useDefinitionEditor } from '../definition-editor/useDefinitionEditor';
import {
  DefinitionEditorActions,
  DefinitionSaveHint,
  DefinitionTemplatesButton,
} from '../definition-editor/DefinitionEditorControls';

export const BlueprintEditorPage = ({
  blueprintId,
  sourceVersion,
}: {
  blueprintId?: string;
  sourceVersion?: number;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const source = useQuery(blueprintRevisionOptions(blueprintId, sourceVersion));
  const sourceRevision = source.data;
  const leaveEditor = () => {
    navigate(
      blueprintId
        ? { params: { blueprintId }, to: '/manage/blueprints/$blueprintId' }
        : { to: '/manage/blueprints' },
    );
  };
  const editor = useDefinitionEditor({
    draftEditor: blueprintId
      ? draftEditors.blueprintRevision
      : draftEditors.blueprintCreate,
    draftResource: blueprintId ? [blueprintId, sourceVersion ?? 0] : [],
    onLeave: leaveEditor,
    ready: !blueprintId || Boolean(sourceRevision),
    source: sourceRevision?.blueprint.definition,
    templates: blueprintTemplates,
  });
  const { definition, isDirty } = editor;

  const save = useMutation({
    mutationFn: ({ definition }: { definition: string }) =>
      blueprintId
        ? createBlueprintRevision(blueprintId, definition)
        : createBlueprint(definition),
    onSuccess: async ({ blueprint }) => {
      editor.clearDraft();
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.catalogue(),
        }),
        queryClient.invalidateQueries({
          queryKey: blueprintQueryKeys.revisions(blueprint.id),
        }),
        // The Explorer, navigation and relationship pickers keep these lists
        // until something in the app invalidates them.
        queryClient.invalidateQueries({
          queryKey: recordQueryKeys.blueprintsByCode(),
        }),
        queryClient.invalidateQueries({
          queryKey: recordQueryKeys.blueprints(),
        }),
      ]);
      navigate({
        params: { blueprintId: blueprint.id },
        to: '/manage/blueprints/$blueprintId',
      });
    },
  });
  const canSave = !save.isPending && (!blueprintId || isDirty);
  const saveDefinition = () => {
    if (canSave && !source.isError) save.mutate({ definition });
  };
  const check = useDefinitionCheck({
    definition,
    kind: definitionKinds.blueprint,
    resourceId: blueprintId,
    validate: (value, id, signal) => validateBlueprint(value, id, signal),
  });

  if (blueprintId && source.isPending) {
    return (
      <PageContainer>
        <Typography>{t('blueprints.loadingBlueprintSource')}</Typography>
      </PageContainer>
    );
  }

  return (
    <PageContainer>
      <PageHeader
        actions={
          <DefinitionEditorActions
            canSave={canSave && !source.isError}
            isDirty={isDirty}
            isSaving={save.isPending}
            onCancel={editor.cancel}
            onSave={saveDefinition}
            saveLabel={
              save.isPending
                ? t('blueprints.saving')
                : t('blueprints.saveDraft')
            }
          />
        }
        description={
          blueprintId
            ? t('blueprints.newRevisionDescription', {
                version: sourceVersion,
              })
            : t('blueprints.newBlueprintDescription')
        }
        icon={BlueprintIcon}
        title={
          blueprintId
            ? t('blueprints.newBlueprintRevision')
            : t('blueprints.newBlueprint')
        }
      />
      {sourceRevision?.attributes.map((attribute) => (
        <ExtensionOutlet
          context={{
            attribute_id: attribute.id,
            blueprint_id: sourceRevision.blueprint.id,
            blueprint_version: sourceRevision.blueprint.version,
          }}
          key={attribute.id}
          outlet="blueprint_attribute_configuration"
        />
      ))}
      {source.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('blueprints.couldNotLoadSource', {
            message: errorMessage(source.error),
          })}
        </Alert>
      )}
      {save.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('blueprints.draftNotSaved', { message: errorMessage(save.error) })}
        </Alert>
      )}
      {editor.dialogs}
      <DefinitionWorkspace
        check={check}
        kind={definitionKinds.blueprint}
        onChange={editor.setDefinition}
        onSave={saveDefinition}
        preview={
          check.result && (
            <BlueprintViewsPreview
              attributes={check.result.attributes}
              definition={definition}
              recordSchema={check.result.blueprint.record_schema}
              views={check.result.blueprint.views}
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

import {
  blueprintCatalogueOptions,
  blueprintRevisionOptions,
} from './queryOptions';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button } from '@mui/material';
import { GitCompareArrowsIcon } from 'lucide-react';
import { compactIconSize } from '../../components/iconSizes';
import { useMemo, useState } from 'react';
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
import {
  blueprintTemplates,
  withUnusedBlueprintCode,
} from './blueprintEditorUtils';
import { BlueprintChangesDialog } from './BlueprintChangesDialog';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';
import { BlueprintIcon } from '../../components/systemIcons';
import { errorMessage } from '../../api/errors';
import { useDefinitionEditor } from '../definition-editor/useDefinitionEditor';
import type { DefinitionTemplate } from '../definition-editor/DefinitionTemplateDialog';
import {
  DefinitionEditorActions,
  DefinitionSaveHint,
  DefinitionTemplatesButton,
} from '../definition-editor/DefinitionEditorControls';
import { extensionOutlets } from '../extensions/constants';
import { attributeLabel } from '../records/recordDisplay';
import { invalidateBlueprintLists } from './invalidateBlueprints';
import { LoadingState } from '../../components/LoadingState';

export const BlueprintEditorPage = ({
  blueprintId,
  sourceVersion,
}: {
  blueprintId?: string;
  sourceVersion?: number;
}) => {
  const { t } = useTranslation();
  // A new blueprint's template gets a code no blueprint uses yet, so the
  // editor waits for the codes before it starts from a template.
  const catalogue = useQuery({
    ...blueprintCatalogueOptions(),
    enabled: !blueprintId,
  });
  const takenCodes = catalogue.data;
  const templates = useMemo(() => {
    const taken = new Set(takenCodes?.map((blueprint) => blueprint.code));
    return blueprintTemplates.map((template) => ({
      ...template,
      definition: withUnusedBlueprintCode(template.definition, taken),
    }));
  }, [takenCodes]);
  if (!blueprintId && catalogue.isPending)
    return (
      <PageContainer>
        <LoadingState>{t('blueprints.loadingBlueprints')}</LoadingState>
      </PageContainer>
    );
  return (
    <BlueprintEditor
      blueprintId={blueprintId}
      sourceVersion={sourceVersion}
      templates={templates}
    />
  );
};

const BlueprintEditor = ({
  blueprintId,
  sourceVersion,
  templates,
}: {
  blueprintId?: string;
  sourceVersion?: number;
  templates: readonly DefinitionTemplate[];
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
    templates,
  });
  const { definition, isDirty } = editor;
  const [reviewOpen, setReviewOpen] = useState(false);
  const savedAttributeCodes = useMemo(
    () => sourceRevision?.attributes.map(({ code }) => code),
    [sourceRevision],
  );

  const save = useMutation({
    mutationFn: ({ definition }: { definition: string }) =>
      blueprintId
        ? createBlueprintRevision(blueprintId, definition)
        : createBlueprint(definition),
    onSuccess: async ({ blueprint }) => {
      editor.clearDraft();
      await invalidateBlueprintLists(queryClient, blueprint.id);
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
        <LoadingState>{t('blueprints.loadingBlueprintSource')}</LoadingState>
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
          heading={attributeLabel(attribute)}
          key={attribute.id}
          outlet={extensionOutlets.blueprintAttributeConfiguration}
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
        savedAttributeCodes={savedAttributeCodes}
        toolbar={
          editor.creating ? (
            <DefinitionTemplatesButton
              disabled={save.isPending}
              onClick={editor.openTemplates}
            />
          ) : (
            <Button
              disabled={!isDirty || !sourceRevision}
              onClick={() => setReviewOpen(true)}
              startIcon={
                <GitCompareArrowsIcon aria-hidden size={compactIconSize} />
              }
            >
              {t('blueprints.review.button')}
            </Button>
          )
        }
        value={definition}
      />
      <DefinitionSaveHint unchanged={!editor.creating && !isDirty} />
      {sourceRevision && savedAttributeCodes && (
        <BlueprintChangesDialog
          checkedAttributeCodes={
            check.result && !check.stale
              ? check.result.attributes.map(({ code }) => code)
              : undefined
          }
          definition={definition}
          error={save.error}
          isPending={save.isPending}
          onClose={() => setReviewOpen(false)}
          onSave={saveDefinition}
          open={reviewOpen}
          saveLabel={
            save.isPending ? t('blueprints.saving') : t('blueprints.saveDraft')
          }
          source={{
            attributeCodes: savedAttributeCodes,
            definition: sourceRevision.blueprint.definition,
            version: sourceRevision.blueprint.version,
          }}
        />
      )}
    </PageContainer>
  );
};

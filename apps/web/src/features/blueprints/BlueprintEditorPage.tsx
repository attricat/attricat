import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Chip, Stack, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { definitionKinds } from '../definition-editor/constants';
import { DefinitionTemplateDialog } from '../definition-editor/DefinitionTemplateDialog';
import { DefinitionWorkspace } from '../definition-editor/DefinitionWorkspace';
import { UnsavedDefinitionChangesDialog } from '../definition-editor/UnsavedDefinitionChangesDialog';
import { useDefinitionCheck } from '../definition-editor/useDefinitionCheck';
import { draftEditors } from '../drafts/constants';
import { DraftRestoreDialog } from '../drafts/DraftRestoreDialog';
import { definitionDraftSchema } from '../drafts/schemas';
import { useEditorDraft } from '../drafts/useEditorDraft';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import {
  createBlueprint,
  createBlueprintRevision,
  getBlueprintRevision,
  validateBlueprint,
} from './api';
import { blueprintTemplates } from './blueprintEditorUtils';
import { BlueprintViewsPreview } from './BlueprintViewsPreview';
import { recordQueryKeys } from '../records/queryKeys';
import { blueprintQueryKeys } from './queryKeys';
import { BlueprintIcon } from '../../components/systemIcons';
import { errorMessage } from '../../api/errors';

type PendingUnsavedAction =
  { templateIndex: number; type: 'replace' } | { type: 'discard' };

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
  const source = useQuery({
    queryKey: blueprintQueryKeys.revision(
      blueprintId ?? '',
      sourceVersion ?? 0,
    ),
    queryFn: () => getBlueprintRevision(blueprintId!, sourceVersion!),
    enabled: Boolean(blueprintId && sourceVersion),
  });
  const [templateIndex, setTemplateIndex] = useState(0);
  const [templateDialogOpen, setTemplateDialogOpen] = useState(!blueprintId);
  const [pendingUnsavedAction, setPendingUnsavedAction] =
    useState<PendingUnsavedAction>();
  const sourceRevision = source.data;
  const initialDefinition =
    sourceRevision?.blueprint.definition ??
    blueprintTemplates[templateIndex].definition;
  const [editedDefinition, setEditedDefinition] = useState<string | null>(null);
  const definition = editedDefinition ?? initialDefinition;
  const isDirty = definition !== initialDefinition;
  const draft = useEditorDraft({
    dirty: isDirty,
    editor: blueprintId
      ? draftEditors.blueprintRevision
      : draftEditors.blueprintCreate,
    ready: !blueprintId || Boolean(sourceRevision),
    resource: blueprintId ? [blueprintId, sourceVersion ?? 0] : [],
    schema: definitionDraftSchema,
    source: sourceRevision?.blueprint.definition ?? null,
    value: definition,
  });

  const save = useMutation({
    mutationFn: ({ definition }: { definition: string }) =>
      blueprintId
        ? createBlueprintRevision(blueprintId, definition)
        : createBlueprint(definition),
    onSuccess: async ({ blueprint }) => {
      draft.clear();
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

  const restoreDraft = () => {
    const restored = draft.restore();
    if (restored === undefined) return;
    setEditedDefinition(restored);
    setTemplateDialogOpen(false);
  };

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

  const leaveEditor = () => {
    navigate(
      blueprintId
        ? { params: { blueprintId }, to: '/manage/blueprints/$blueprintId' }
        : { to: '/manage/blueprints' },
    );
  };

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

  if (blueprintId && source.isPending) {
    return (
      <PageContainer>
        <Typography>{t('blueprints.loadingBlueprintSource')}</Typography>
      </PageContainer>
    );
  }

  return (
    <PageContainer>
      <Stack
        direction={{ xs: 'column', sm: 'row' }}
        spacing={2}
        sx={{ alignItems: { sm: 'center' }, justifyContent: 'space-between' }}
      >
        <PageHeader
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
        <Stack direction="row" spacing={1}>
          {isDirty && (
            <Chip color="warning" label={t('definitionEditor.unsaved.chip')} />
          )}
          <Button disabled={save.isPending} onClick={cancel}>
            {t('blueprints.cancel')}
          </Button>
          <Button
            disabled={!canSave || source.isError}
            onClick={saveDefinition}
            variant="contained"
          >
            {save.isPending
              ? t('blueprints.saving')
              : t('blueprints.saveDraft')}
          </Button>
        </Stack>
      </Stack>
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
      {!blueprintId && (
        <DefinitionTemplateDialog
          onClose={() => setTemplateDialogOpen(false)}
          onSelect={selectTemplate}
          open={templateDialogOpen && !draft.pending}
          selectedIndex={templateIndex}
          templates={blueprintTemplates}
        />
      )}
      <DraftRestoreDialog
        draft={draft.pending}
        onDiscard={draft.discard}
        onRestore={restoreDraft}
      />
      <UnsavedDefinitionChangesDialog
        action={pendingUnsavedAction?.type}
        onCancel={() => setPendingUnsavedAction(undefined)}
        onConfirm={confirmUnsavedAction}
      />
      <DefinitionWorkspace
        check={check}
        kind={definitionKinds.blueprint}
        onChange={setEditedDefinition}
        onSave={saveDefinition}
        preview={
          check.result && (
            <BlueprintViewsPreview
              attributes={check.result.attributes}
              recordSchema={check.result.blueprint.record_schema}
              views={check.result.blueprint.views}
            />
          )
        }
        readOnly={save.isPending}
        toolbar={
          !blueprintId && (
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
        {blueprintId && !isDirty
          ? t('definitionEditor.makeChangeBeforeSaving')
          : t('definitionEditor.saveShortcut')}
      </Typography>
    </PageContainer>
  );
};

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, Chip, Stack, Typography } from '@mui/material';
import { useLayoutEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { TomlEditor } from '../../components/TomlEditor';
import { draftEditors } from '../drafts/constants';
import { DraftRestoreDialog } from '../drafts/DraftRestoreDialog';
import { definitionDraftSchema } from '../drafts/schemas';
import { useEditorDraft } from '../drafts/useEditorDraft';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import {
  createBlueprint,
  createBlueprintRevision,
  getBlueprintRevision,
} from './api';
import { blueprintTemplates } from './blueprintEditorUtils';
import { BlueprintTemplateDialog } from './BlueprintTemplateDialog';
import { blueprintEditorHeight } from './constants';
import { blueprintQueryKeys } from './queryKeys';
import { UnsavedBlueprintChangesDialog } from './UnsavedBlueprintChangesDialog';

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
      ]);
      navigate({
        params: { blueprintId: blueprint.id },
        to: '/manage/blueprints/$blueprintId',
      });
    },
  });
  const canSave = !save.isPending && (!blueprintId || isDirty);

  // Monaco keeps the handlers registered on mount, so they read the latest
  // render's values through refs instead of capturing stale ones.
  const savePendingRef = useRef(save.isPending);
  const saveShortcutRef = useRef<() => void>(() => undefined);
  useLayoutEffect(() => {
    savePendingRef.current = save.isPending;
    saveShortcutRef.current = () => {
      if (canSave) save.mutate({ definition });
    };
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
          title={
            blueprintId
              ? t('blueprints.newBlueprintRevision')
              : t('blueprints.newBlueprint')
          }
        />
        <Stack direction="row" spacing={1}>
          {!blueprintId && (
            <Button
              disabled={save.isPending}
              onClick={() => setTemplateDialogOpen(true)}
            >
              {t('blueprints.examples')}
            </Button>
          )}
          {isDirty && (
            <Chip color="warning" label={t('blueprints.unsavedChanges')} />
          )}
          <Button disabled={save.isPending} onClick={cancel}>
            {t('blueprints.cancel')}
          </Button>
          <Button
            disabled={!canSave || source.isError}
            onClick={() => save.mutate({ definition })}
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
            message: source.error.message,
          })}
        </Alert>
      )}
      {save.isError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {t('blueprints.draftNotSaved', { message: save.error.message })}
        </Alert>
      )}
      {!blueprintId && (
        <BlueprintTemplateDialog
          onClose={() => setTemplateDialogOpen(false)}
          onSelect={selectTemplate}
          open={templateDialogOpen && !draft.pending}
          selectedIndex={templateIndex}
        />
      )}
      <DraftRestoreDialog
        draft={draft.pending}
        onDiscard={draft.discard}
        onRestore={restoreDraft}
      />
      <UnsavedBlueprintChangesDialog
        action={pendingUnsavedAction?.type}
        onCancel={() => setPendingUnsavedAction(undefined)}
        onConfirm={confirmUnsavedAction}
      />
      <TomlEditor
        height={blueprintEditorHeight}
        marginTop={3}
        onChange={(value) => {
          if (!savePendingRef.current) setEditedDefinition(value ?? '');
        }}
        onMount={(editor, monaco) => {
          editor.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.KeyS, () =>
            saveShortcutRef.current(),
          );
        }}
        readOnly={save.isPending}
        value={definition}
      />
      <Typography color="text.secondary" sx={{ mt: 1 }} variant="body2">
        {blueprintId && !isDirty
          ? t('blueprints.makeChangeBeforeSaving')
          : t('blueprints.saveShortcut')}
      </Typography>
    </PageContainer>
  );
};

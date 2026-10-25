import { useState } from 'react';
import type { DraftEditor } from '../drafts/constants';
import { DraftRestoreDialog } from '../drafts/DraftRestoreDialog';
import { definitionDraftSchema } from '../drafts/schemas';
import { useEditorDraft } from '../drafts/useEditorDraft';
import {
  DefinitionTemplateDialog,
  type DefinitionTemplate,
} from './DefinitionTemplateDialog';
import { UnsavedDefinitionChangesDialog } from './UnsavedDefinitionChangesDialog';

type PendingUnsavedAction =
  { templateIndex: number; type: 'replace' } | { type: 'discard' };

/**
 * The state of a page that writes a new definition from a template, or a new
 * revision of `source`: the edited text, its unsaved draft, and the
 * confirmations before a template or Cancel would drop changes.
 */
export const useDefinitionEditor = ({
  draftEditor,
  draftResource,
  onLeave,
  ready,
  source,
  templates,
}: {
  draftEditor: DraftEditor;
  /** The revised resource; empty when writing a new definition. */
  draftResource: readonly (string | number)[];
  onLeave: () => void;
  /** Whether `source` has loaded when revising. */
  ready: boolean;
  source: string | undefined;
  templates: readonly DefinitionTemplate[];
}) => {
  const creating = draftResource.length === 0;
  const [templateIndex, setTemplateIndex] = useState(0);
  const [templateDialogOpen, setTemplateDialogOpen] = useState(creating);
  const [pendingUnsavedAction, setPendingUnsavedAction] =
    useState<PendingUnsavedAction>();
  const [editedDefinition, setEditedDefinition] = useState<string | null>(null);
  const initialDefinition = source ?? templates[templateIndex].definition;
  const definition = editedDefinition ?? initialDefinition;
  const isDirty = definition !== initialDefinition;
  const draft = useEditorDraft({
    dirty: isDirty,
    editor: draftEditor,
    ready,
    resource: draftResource,
    schema: definitionDraftSchema,
    source: source ?? null,
    value: definition,
  });

  const applyTemplate = (index: number) => {
    setTemplateIndex(index);
    setEditedDefinition(null);
    setTemplateDialogOpen(false);
  };
  const confirmUnsavedAction = () => {
    if (pendingUnsavedAction?.type === 'replace')
      applyTemplate(pendingUnsavedAction.templateIndex);
    else if (pendingUnsavedAction?.type === 'discard') onLeave();
    setPendingUnsavedAction(undefined);
  };

  return {
    cancel: () => {
      if (isDirty) setPendingUnsavedAction({ type: 'discard' });
      else onLeave();
    },
    creating,
    definition,
    /** Clears the saved draft once the definition is stored. */
    clearDraft: draft.clear,
    isDirty,
    openTemplates: () => setTemplateDialogOpen(true),
    setDefinition: setEditedDefinition,
    dialogs: (
      <>
        {creating && (
          <DefinitionTemplateDialog
            onClose={() => setTemplateDialogOpen(false)}
            onSelect={(index) => {
              if (isDirty)
                setPendingUnsavedAction({
                  templateIndex: index,
                  type: 'replace',
                });
              else applyTemplate(index);
            }}
            open={templateDialogOpen && !draft.pending}
            selectedIndex={templateIndex}
            templates={templates}
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
      </>
    ),
  };
};

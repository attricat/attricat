import { useMemo, useState } from 'react';
import { z } from 'zod';
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
export const useDefinitionEditor = <Scope = never,>({
  draftEditor,
  draftScope,
  draftResource,
  onLeave,
  ready,
  source,
  templates,
  templatesReady = true,
}: {
  /**
   * Whether choosing a template makes sense yet, such as once the blueprint
   * it would check is chosen; the dialog opens then.
   */
  templatesReady?: boolean;
  draftEditor: DraftEditor;
  /**
   * What else the definition depends on, kept in its draft and given back on
   * restore. Keep `schema` referentially stable.
   */
  draftScope?: {
    schema: z.ZodType<Scope>;
    value: Scope;
    onRestore: (scope: Scope) => void;
  };
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
  const scopeSchema = draftScope?.schema;
  const scopeValue = draftScope?.value;
  const draftSchema = useMemo(
    () =>
      scopeSchema
        ? z.object({ definition: definitionDraftSchema, scope: scopeSchema })
        : definitionDraftSchema,
    [scopeSchema],
  ) as z.ZodType<string | { definition: string; scope: Scope }>;
  const draftValue = useMemo(
    () =>
      scopeSchema ? { definition, scope: scopeValue as Scope } : definition,
    [definition, scopeSchema, scopeValue],
  );
  const draft = useEditorDraft({
    dirty: isDirty,
    editor: draftEditor,
    ready,
    resource: draftResource,
    schema: draftSchema,
    source: source ?? null,
    value: draftValue,
  });

  const applyTemplate = (index: number) => {
    setTemplateIndex(index);
    setEditedDefinition(null);
    setTemplateDialogOpen(false);
  };
  const confirmUnsavedAction = () => {
    if (pendingUnsavedAction?.type === 'replace')
      applyTemplate(pendingUnsavedAction.templateIndex);
    else if (pendingUnsavedAction?.type === 'discard') {
      draft.clear();
      onLeave();
    }
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
            open={templateDialogOpen && templatesReady && !draft.pending}
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
            if (typeof restored === 'string') setEditedDefinition(restored);
            else {
              setEditedDefinition(restored.definition);
              draftScope?.onRestore(restored.scope);
            }
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

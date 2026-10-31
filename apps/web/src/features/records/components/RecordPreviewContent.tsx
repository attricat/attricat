import { Alert, Box, CircularProgress, Typography } from '@mui/material';
import { useMemo, useRef, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import type { AttributeContext } from '../../contexts/api';
import { extensionOutlets } from '../../extensions/constants';
import { ExtensionActionButtons } from '../../extensions/ExtensionActions';
import { FieldNotesProvider } from '../../extensions/fieldNotes';
import { RecordView } from '../../views/components/RecordView';
import { recordHeadingComponentId } from '../../views/components/blocks/RecordHeadingDefinition';
import type {
  RecordFormResponse,
  getBlueprintRevision,
  getResolvedRecordPreview,
  StatusTransitionAccess,
} from '../api';
import { FieldDiagnosticsContext } from '../fieldDiagnostics';
import { useBlueprintFieldDiagnostics } from '../useBlueprintFieldDiagnostics';
import { RecordContextPicker } from './RecordContextPicker';
import {
  RecordInlineFields,
  type RecordInlineFieldsHandle,
} from './RecordInlineFields';
import { usePaletteSurface } from '../../extensions/actionPaletteStore';

type Props = {
  blueprint: Awaited<ReturnType<typeof getBlueprintRevision>>;
  contextId?: string;
  contexts: readonly AttributeContext[];
  defaultContextId: string | null;
  recordId: string;
  /** The editable form; values are shown read-only only if it fails to load. */
  form?: RecordFormResponse;
  /** No form has arrived yet; the fields wait for it instead of swapping. */
  formPending?: boolean;
  /** `form` is a cached copy that the opening refresh has not replaced yet. */
  formRefreshing?: boolean;
  inlineFieldsRef?: Ref<RecordInlineFieldsHandle>;
  statusParentContextIds: readonly string[];
  statusTransitions?: readonly StatusTransitionAccess[];
  onContextChange: (contextId: string) => void;
  resolved: Awaited<ReturnType<typeof getResolvedRecordPreview>>;
  /** Lays the view out in one column, as in a narrow panel. */
  singleColumn?: boolean;
};

/** Resolved attribute values of a record for the selected context. */
export const RecordPreviewContent = ({
  blueprint,
  contextId,
  contexts,
  defaultContextId,
  recordId,
  form,
  formPending = false,
  formRefreshing = false,
  inlineFieldsRef,
  statusParentContextIds,
  statusTransitions,
  onContextChange,
  resolved,
  singleColumn,
}: Props) => {
  const { t } = useTranslation();
  // Switching context would drop changes that are not saved yet, and
  // choosing another context is often what commits the last edit. The choice
  // waits for those changes to save rather than being lost to a disabled tab.
  const hasPendingChanges = useRef(false);
  const queuedContextId = useRef<string>(undefined);
  const changeContext = (nextContextId: string) => {
    if (hasPendingChanges.current) queuedContextId.current = nextContextId;
    else onContextChange(nextContextId);
  };
  const pendingChanged = (pending: boolean) => {
    hasPendingChanges.current = pending;
    const queued = queuedContextId.current;
    if (pending || queued === undefined) return;
    queuedContextId.current = undefined;
    onContextChange(queued);
  };
  const shownContextId = contextId ?? defaultContextId ?? undefined;
  const diagnostics = useBlueprintFieldDiagnostics({
    attributes: blueprint.attributes,
    recordSchema: blueprint.blueprint.record_schema,
    definition: blueprint.blueprint.definition,
    revision: blueprint.blueprint,
    record: {
      id: recordId,
      contextCode: contexts.find((context) => context.id === shownContextId)
        ?.code,
      contextId: shownContextId,
    },
  });
  const runtimeScope = {
    blueprintId: blueprint.blueprint.id,
    blueprintVersion: blueprint.blueprint.version,
  };
  // Stable while the form is, so the field notes built on it are too.
  const recordTags = useMemo(
    () => (form ? [form.record.system_tags ?? []] : undefined),
    [form],
  );
  const recordActionTarget = {
    blueprintId: blueprint.blueprint.id,
    blueprintVersion: blueprint.blueprint.version,
    blueprintCode: blueprint.blueprint.code,
    contextId: contextId ?? null,
    recordIds: [recordId],
    canWrite: form?.can_write,
    recordTags,
  };
  usePaletteSurface('record-action', {
    outlet: extensionOutlets.recordAction,
    runtimeScope,
    target: recordActionTarget,
  });
  return (
    <>
      <FieldDiagnosticsContext value={diagnostics}>
        <FieldNotesProvider
          conditions={recordActionTarget}
          contextId={contextId ?? null}
          recordId={recordId}
          runtimeScope={runtimeScope}
        >
          <RecordContextPicker
            contexts={contexts}
            onChange={changeContext}
            value={contextId ?? ''}
          />
          {form && contextId && contextId !== defaultContextId && (
            // Says whose values the fields change, and where the rest come from.
            <Alert severity="info" sx={{ mt: 2 }}>
              {t('records.editingContext', {
                context:
                  contexts.find((context) => context.id === contextId)?.code ??
                  contextId,
              })}
            </Alert>
          )}
          {form ? (
            <RecordInlineFields
              key={`${recordId}:${contextId ?? ''}`}
              attributes={blueprint.attributes}
              contextId={contextId ?? null}
              defaultContextId={defaultContextId}
              recordId={recordId}
              form={form}
              ref={inlineFieldsRef}
              onPendingChange={pendingChanged}
              refreshing={formRefreshing}
              resolvedValues={resolved.values}
              reusableResolvedValues={resolved.reusable_values ?? {}}
              singleColumn={singleColumn}
              statusParentContextIds={statusParentContextIds}
              statusTransitions={statusTransitions}
              view={blueprint.blueprint.views.detail}
            />
          ) : formPending ? (
            <Box sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}>
              <CircularProgress
                aria-label={t('records.loadingFields')}
                enableTrackSlot
              />
            </Box>
          ) : (
            <>
              <RecordView
                attributes={blueprint.attributes}
                fallbackVisibilityScope="detail"
                contextId={contextId}
                recordId={recordId}
                values={resolved.values}
                view={blueprint.blueprint.views.detail}
                singleColumn={singleColumn}
                skipComponentId={recordHeadingComponentId}
              />
              {(resolved.reusable_attributes?.length ?? 0) > 0 && (
                <Box component="section" sx={{ mt: 4 }}>
                  <Typography component="h2" variant="h6">
                    {t('records.additionalAttributes')}
                  </Typography>
                  <RecordView
                    attributes={resolved.reusable_attributes}
                    contextId={contextId}
                    recordId={recordId}
                    singleColumn={singleColumn}
                    values={resolved.reusable_values ?? {}}
                  />
                </Box>
              )}
            </>
          )}
        </FieldNotesProvider>
      </FieldDiagnosticsContext>
      {/* Set apart from the fields above; collapses without contributions. */}
      <Box sx={{ mt: 6, '&:empty': { display: 'none' } }}>
        <ExtensionActionButtons
          outlet={extensionOutlets.recordAction}
          runtimeScope={runtimeScope}
          target={recordActionTarget}
        />
      </Box>
    </>
  );
};

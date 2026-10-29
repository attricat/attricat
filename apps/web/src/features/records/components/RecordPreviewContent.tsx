import { Alert, Box, CircularProgress, Typography } from '@mui/material';
import { useState, type Ref } from 'react';
import { useTranslation } from 'react-i18next';
import type { AttributeContext } from '../../contexts/api';
import {
  ExtensionOutlet,
  ExtensionPopoverOutlet,
} from '../../extensions/ExtensionOutlet';
import {
  selectionSources,
  supportedOutletContextVersion,
  contributionKinds,
  extensionOutlets,
} from '../../extensions/constants';
import { useExtensionRuntime } from '../../extensions/useExtensionRuntime';
import { RecordView } from '../../views/components/RecordView';
import { recordHeadingComponentId } from '../../views/components/blocks/RecordHeadingDefinition';
import type {
  Attribute,
  RecordFormResponse,
  getBlueprintRevision,
  getResolvedRecordPreview,
  StatusTransitionAccess,
} from '../api';
import { FieldDiagnosticsContext } from '../fieldDiagnostics';
import { useBlueprintFieldDiagnostics } from '../useBlueprintFieldDiagnostics';
import { attributeLabel } from '../recordDisplay';
import { RecordContextPicker } from './RecordContextPicker';
import {
  RecordInlineFields,
  type RecordInlineFieldsHandle,
} from './RecordInlineFields';

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
  // Switching context would drop changes that are not saved yet.
  const [hasPendingChanges, setHasPendingChanges] = useState(false);
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
  const runtime = useExtensionRuntime(runtimeScope);
  const hasAttributePanels = runtime.data?.some(
    (item) =>
      item.outlet === 'record_attribute_panel' &&
      item.kind === contributionKinds.panel,
  );
  const hasAttributeDecorations = runtime.data?.some(
    (item) =>
      item.outlet === extensionOutlets.recordAttributeDecoration &&
      item.kind === contributionKinds.embedded,
  );
  const hasFilePanels = runtime.data?.some(
    (item) =>
      item.outlet === 'file_panel' && item.kind === contributionKinds.panel,
  );
  // Fields reserve a decoration slot only while a contribution can fill it.
  const renderAttributeDecoration = hasAttributeDecorations
    ? (attribute: Attribute) => (
        <ExtensionPopoverOutlet
          context={{
            attribute_id: attribute.id,
            blueprint_id: blueprint.blueprint.id,
            blueprint_version: blueprint.blueprint.version,
            context_id: contextId,
            record_id: recordId,
          }}
          key={String(attribute.id)}
          label={t('records.viewExtensionContent', {
            attribute: attributeLabel(attribute),
          })}
          outlet={extensionOutlets.recordAttributeDecoration}
          runtimeScope={runtimeScope}
        />
      )
    : undefined;
  const renderAttributePanel = hasAttributePanels
    ? (attribute: Attribute) => (
        <ExtensionOutlet
          context={{
            context_version: supportedOutletContextVersion,
            record_id: recordId,
            attribute_id: attribute.id,
            blueprint_id: blueprint.blueprint.id,
            blueprint_version: blueprint.blueprint.version,
            context_id: contextId ?? null,
          }}
          outlet={extensionOutlets.recordAttributePanel}
          runtimeScope={runtimeScope}
        />
      )
    : undefined;
  const renderFilePanel = hasFilePanels
    ? (attribute: Attribute, fileId: string) => (
        <ExtensionOutlet
          context={{
            context_version: supportedOutletContextVersion,
            file_id: fileId,
            record_id: recordId,
            attribute_id: attribute.id,
            blueprint_id: blueprint.blueprint.id,
            blueprint_version: blueprint.blueprint.version,
          }}
          outlet={extensionOutlets.filePanel}
          runtimeScope={runtimeScope}
        />
      )
    : undefined;
  return (
    <>
      <FieldDiagnosticsContext value={diagnostics}>
        <RecordContextPicker
          contexts={contexts}
          disabled={hasPendingChanges}
          onChange={onContextChange}
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
            onPendingChange={setHasPendingChanges}
            refreshing={formRefreshing}
            renderAttributeDecoration={renderAttributeDecoration}
            renderAttributePanel={renderAttributePanel}
            renderFilePanel={renderFilePanel}
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
              renderAttributeDecoration={renderAttributeDecoration}
              renderAttributePanel={renderAttributePanel}
              renderFilePanel={renderFilePanel}
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
      </FieldDiagnosticsContext>
      {/* Set apart from the fields above; collapses without contributions. */}
      <Box sx={{ mt: 6, '&:empty': { display: 'none' } }}>
        <ExtensionOutlet
          context={{ record_id: recordId, context_id: contextId }}
          outlet={extensionOutlets.recordAction}
          runtimeScope={runtimeScope}
          selection={{
            source: selectionSources.recordPreview,
            blueprintId: blueprint.blueprint.id,
            blueprintVersion: blueprint.blueprint.version,
            contextId: contextId ?? null,
            recordIds: [recordId],
          }}
        />
      </Box>
    </>
  );
};

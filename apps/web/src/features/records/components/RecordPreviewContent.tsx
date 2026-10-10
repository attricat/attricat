import { Box, Paper, Typography } from '@mui/material';
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
  /** The editable form; values are shown read-only until it is loaded. */
  form?: RecordFormResponse;
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
  const runtimeScope = {
    blueprintId: blueprint.blueprint.id,
    blueprintVersion: blueprint.blueprint.version,
  };
  const runtime = useExtensionRuntime(runtimeScope);
  const hasAttributePanels = runtime.data?.some(
    (item) => item.outlet === 'record_attribute_panel' && item.kind === 'panel',
  );
  const hasFilePanels = runtime.data?.some(
    (item) => item.outlet === 'file_panel' && item.kind === 'panel',
  );
  const renderAttributeDecoration = (attribute: Attribute) => (
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
      outlet="record_attribute_decoration"
      runtimeScope={runtimeScope}
    />
  );
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
          outlet="record_attribute_panel"
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
          outlet="file_panel"
          runtimeScope={runtimeScope}
        />
      )
    : undefined;
  return (
    <Box sx={{ mt: 3 }}>
      <Paper component="section" sx={{ p: { xs: 2, md: 3 } }}>
        <RecordContextPicker
          contexts={contexts}
          disabled={hasPendingChanges}
          onChange={onContextChange}
          value={contextId ?? ''}
        />
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
      </Paper>
      <ExtensionOutlet
        context={{ record_id: recordId, context_id: contextId }}
        outlet="record_action"
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
  );
};

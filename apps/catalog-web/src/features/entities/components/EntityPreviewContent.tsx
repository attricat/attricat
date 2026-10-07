import { Box, Paper, Typography } from '@mui/material';
import { useState } from 'react';
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
import { EntityView } from '../../views/components/EntityView';
import { entityHeadingComponentId } from '../../views/components/blocks/EntityHeadingDefinition';
import type {
  Attribute,
  EntityFormResponse,
  getBlueprintRevision,
  getResolvedEntityPreview,
  StatusTransitionAccess,
} from '../api';
import { attributeLabel } from '../entityDisplay';
import { EntityContextPicker } from './EntityContextPicker';
import { EntityInlineFields } from './EntityInlineFields';

type Props = {
  blueprint: Awaited<ReturnType<typeof getBlueprintRevision>>;
  contextId?: string;
  contexts: readonly AttributeContext[];
  contextsPending: boolean;
  defaultContextId: string | null;
  entityId: string;
  /** The editable form; values are shown read-only until it is loaded. */
  form?: EntityFormResponse;
  statusParentContextIds: readonly string[];
  statusTransitions?: readonly StatusTransitionAccess[];
  onContextChange: (contextId: string) => void;
  resolved: Awaited<ReturnType<typeof getResolvedEntityPreview>>;
};

/** Resolved attribute values of an entity for the selected context. */
export const EntityPreviewContent = ({
  blueprint,
  contextId,
  contexts,
  contextsPending,
  defaultContextId,
  entityId,
  form,
  statusParentContextIds,
  statusTransitions,
  onContextChange,
  resolved,
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
    (item) => item.outlet === 'entity_attribute_panel' && item.kind === 'panel',
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
        entity_id: entityId,
      }}
      key={String(attribute.id)}
      label={t('entities.viewExtensionContent', {
        attribute: attributeLabel(attribute),
      })}
      outlet="entity_attribute_decoration"
      runtimeScope={runtimeScope}
    />
  );
  const renderAttributePanel = hasAttributePanels
    ? (attribute: Attribute) => (
        <ExtensionOutlet
          context={{
            context_version: supportedOutletContextVersion,
            entity_id: entityId,
            attribute_id: attribute.id,
            blueprint_id: blueprint.blueprint.id,
            blueprint_version: blueprint.blueprint.version,
            context_id: contextId ?? null,
          }}
          outlet="entity_attribute_panel"
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
            entity_id: entityId,
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
    <Box
      sx={{
        display: 'grid',
        gap: 3,
        gridTemplateColumns: 'minmax(0, 1fr)',
        mt: 3,
      }}
    >
      <Box>
        <Paper component="section" sx={{ p: { xs: 2, md: 3 } }}>
          <EntityContextPicker
            contexts={contexts}
            disabled={contextsPending || hasPendingChanges}
            onChange={onContextChange}
            value={contextId ?? ''}
          />
          {form ? (
            <EntityInlineFields
              key={`${entityId}:${contextId ?? ''}`}
              attributes={blueprint.attributes}
              contextId={contextId ?? null}
              defaultContextId={defaultContextId}
              entityId={entityId}
              form={form}
              onPendingChange={setHasPendingChanges}
              renderAttributeDecoration={renderAttributeDecoration}
              renderAttributePanel={renderAttributePanel}
              renderFilePanel={renderFilePanel}
              resolvedValues={resolved.values}
              reusableResolvedValues={resolved.reusable_values ?? {}}
              statusParentContextIds={statusParentContextIds}
              statusTransitions={statusTransitions}
              view={blueprint.blueprint.views.detail}
            />
          ) : (
            <>
              <EntityView
                attributes={blueprint.attributes}
                fallbackVisibilityScope="detail"
                contextId={contextId}
                entityId={entityId}
                renderAttributeDecoration={renderAttributeDecoration}
                renderAttributePanel={renderAttributePanel}
                renderFilePanel={renderFilePanel}
                values={resolved.values}
                view={blueprint.blueprint.views.detail}
                skipComponentId={entityHeadingComponentId}
              />
              {(resolved.reusable_attributes?.length ?? 0) > 0 && (
                <Box component="section" sx={{ mt: 4 }}>
                  <Typography component="h2" variant="h6">
                    {t('entities.additionalAttributes')}
                  </Typography>
                  <EntityView
                    attributes={resolved.reusable_attributes}
                    contextId={contextId}
                    entityId={entityId}
                    values={resolved.reusable_values ?? {}}
                  />
                </Box>
              )}
            </>
          )}
        </Paper>
        <ExtensionOutlet
          context={{ entity_id: entityId, context_id: contextId }}
          outlet="entity_action"
          runtimeScope={runtimeScope}
          selection={{
            source: selectionSources.entityPreview,
            blueprintId: blueprint.blueprint.id,
            blueprintVersion: blueprint.blueprint.version,
            contextId: contextId ?? null,
            entityIds: [entityId],
          }}
        />
      </Box>
    </Box>
  );
};

import { useQuery } from '@tanstack/react-query';
import { Alert, Box, CircularProgress } from '@mui/material';
import { lazy, Suspense, useRef, useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { QueryErrorNotice } from '../../../components/QueryErrorNotice';
import { ApiErrorAlert } from '../../../components/CheckViolationsAlert';
import { ExtensionOutlet } from '../../extensions/ExtensionOutlet';
import { currentSession } from '../../auth/api';
import { authQueryKeys } from '../../auth/queryKeys';
import { lexiconText } from '../../lexicon/lexicon';
import { getCurrentBlueprint, type Entity } from '../api';
import {
  ENTITY_HEADER_CONTEXT_VERSION,
  FALLBACK_BLUEPRINT_VERSION,
} from '../constants';
import { entityQueryKeys } from '../queryKeys';
import { statusParentContexts } from '../status';
import { useEntityContextSelection } from '../useEntityContexts';
import { useEntityPreviewData } from '../useEntityPreviewData';
import { DataQualityFindingsChip } from './DataQualityFindingsChip';
import { DeleteEntityDialog } from './DeleteEntityDialog';
import { DuplicateEntityDialog } from './DuplicateEntityDialog';
import { EntityAgentDrawer } from './EntityAgentDrawer';
import { EntityBlueprintHeaderActions } from './EntityBlueprintHeaderActions';
import { EntityExtensionDrawer } from './EntityExtensionDrawer';
import { EntityHeading } from './EntityHeading';
import type { EntityInlineFieldsHandle } from './EntityInlineFields';
import { EntityPreviewContent } from './EntityPreviewContent';
import { EntityPreviewToolbar } from './EntityPreviewToolbar';
import { EntitySchemaSubheader } from './EntitySchemaSubheader';
import { RecordControlsPanel } from './RecordControlsPanel';
import { RelationshipPickerActionBar } from './RelationshipPickerActionBar';
import { useEntityPublications } from './useEntityPublications';

const EntityCommentsPanel = lazy(() =>
  import('../../entity-comments/EntityCommentsPanel').then((module) => ({
    default: module.EntityCommentsPanel,
  })),
);

export type EntityPreviewFrame = {
  /** The entity's blueprint revision, once loaded. */
  blueprint?: { code: string; id: string; name: string; version: number };
  /** Blueprint, sample, schema and data quality indicators. */
  headerActions: ReactNode;
  children: ReactNode;
};

type Props = {
  entityId: string;
  /** Renders smaller headings and one column of fields for a panel beside another page. */
  compact?: boolean;
  /** The context shown until another is chosen. */
  initialContextId?: string;
  /** Right edge of the agent and extension drawers, beside a panel. */
  drawerOffset?: string;
  relationshipPickerToken?: string;
  /** Comments are shown only on the entity page for now. */
  showComments?: boolean;
  onDeleted: () => void;
  onDuplicated: (copy: Entity) => void;
  /** Places the preview in its page or panel. */
  renderFrame: (frame: EntityPreviewFrame) => ReactNode;
};

/**
 * An entity's values, actions and side panels. The entity page and the
 * Explorer's entity panel differ only in their frame.
 */
export const EntityPreview = ({
  entityId,
  compact = false,
  initialContextId,
  drawerOffset,
  relationshipPickerToken,
  showComments = false,
  onDeleted,
  onDuplicated,
  renderFrame,
}: Props) => {
  const { t } = useTranslation();
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [duplicateOpen, setDuplicateOpen] = useState(false);
  const [extensionPanelOpen, setExtensionPanelOpen] = useState(false);
  const [agentPanelOpen, setAgentPanelOpen] = useState(false);
  const inlineFieldsRef = useRef<EntityInlineFieldsHandle>(null);
  const { contextId, contexts, defaultContextId, setSelectedContext } =
    useEntityContextSelection(initialContextId);
  const selectedContextId = contextId ?? undefined;
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const canPublish = session.data?.capabilities?.entities_publish === true;
  const publications = useEntityPublications(entityId, contextId, canPublish);
  const { blueprint, entityForm, resolved, statusTransitions } =
    useEntityPreviewData(entityId, contextId);
  const resolvedEntity = resolved.data?.entity;
  const currentBlueprint = useQuery({
    queryKey: entityQueryKeys.currentBlueprint(
      resolvedEntity?.blueprint_id ?? '',
    ),
    queryFn: () => {
      if (!resolvedEntity)
        throw new Error(t('entities.entityPreviewUnavailable'));
      return getCurrentBlueprint(resolvedEntity.blueprint_id);
    },
    enabled: Boolean(resolvedEntity?.blueprint_id),
  });
  const schemaOutdated =
    currentBlueprint.data && resolvedEntity
      ? currentBlueprint.data.blueprint.version >
        resolvedEntity.blueprint_version
      : undefined;
  const actionError = publications.error;
  const loaded = resolved.data && blueprint.data;

  return (
    <>
      {renderFrame({
        blueprint: blueprint.data?.blueprint,
        headerActions: (
          <EntityBlueprintHeaderActions
            blueprint={blueprint.data?.blueprint}
            hideBlueprintLink={compact}
            schemaOutdated={schemaOutdated}
            isSample={resolvedEntity?.is_sample}
          >
            <DataQualityFindingsChip entityId={entityId} />
          </EntityBlueprintHeaderActions>
        ),
        children: (
          <>
            {relationshipPickerToken && (
              <RelationshipPickerActionBar
                entityId={entityId}
                pickerToken={relationshipPickerToken}
              />
            )}
            {resolved.data && blueprint.data && (
              <EntityHeading
                attributes={blueprint.data.attributes}
                compact={compact}
                entityId={entityId}
                values={resolved.data.values}
                view={blueprint.data.blueprint.views.detail}
              />
            )}
            {actionError && (
              <ApiErrorAlert error={actionError} sx={{ mt: 3 }} />
            )}
            {blueprint.data && resolved.data && (
              <ExtensionOutlet
                context={{
                  context_version: ENTITY_HEADER_CONTEXT_VERSION,
                  entity_id: entityId,
                  blueprint_id: blueprint.data.blueprint.id,
                  blueprint_version: blueprint.data.blueprint.version,
                }}
                outlet="entity_header_action"
                runtimeScope={{
                  blueprintId: blueprint.data.blueprint.id,
                  blueprintVersion: blueprint.data.blueprint.version,
                }}
              />
            )}
            <EntityPreviewToolbar
              compact={compact}
              entityId={entityId}
              extensionPanelOpen={extensionPanelOpen}
              onOpenExtensions={() => {
                setAgentPanelOpen(false);
                setExtensionPanelOpen(true);
              }}
              agentPanelOpen={agentPanelOpen}
              onOpenAgent={() => {
                setExtensionPanelOpen(false);
                setAgentPanelOpen(true);
              }}
              schemaOutdated={schemaOutdated}
              showExtensions={Boolean(loaded)}
              onDuplicate={() => setDuplicateOpen(true)}
              canDelete={
                session.data?.capabilities?.entities_delete === true &&
                Boolean(resolved.data)
              }
              onDelete={() => setDeleteOpen(true)}
              publication={publications.publication}
              canPublish={canPublish}
              readiness={publications.readiness}
              notReadyChannels={publications.notReadyChannels}
              onPublish={publications.publish}
              onPublishAll={publications.publishAll}
              onUnpublish={publications.unpublish}
              publicationPending={publications.isPending}
            />
            <EntitySchemaSubheader
              compact={compact}
              entityId={entityId}
              name={
                blueprint.data && lexiconText(blueprint.data.blueprint.name)
              }
            />
            {contexts.isPending && (
              <Box sx={{ display: 'flex', justifyContent: 'center', py: 3 }}>
                <CircularProgress
                  aria-label={t('entities.loadingContexts')}
                  enableTrackSlot
                />
              </Box>
            )}
            <QueryErrorNotice
              error={contexts.error}
              isRetrying={contexts.isFetching}
              onRetry={() => void contexts.refetch()}
            />
            {contexts.data && (
              <>
                {resolved.isPending && (
                  <Box
                    sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}
                  >
                    <CircularProgress
                      aria-label={t('entities.resolvingValues')}
                      enableTrackSlot
                    />
                  </Box>
                )}
                {resolved.isError && (
                  <Alert severity="error" sx={{ mt: 3 }}>
                    {resolved.error.message}
                  </Alert>
                )}
                {blueprint.isError && (
                  <Alert severity="error" sx={{ mt: 3 }}>
                    {blueprint.error.message}
                  </Alert>
                )}
                {resolved.data && blueprint.data && (
                  <EntityPreviewContent
                    blueprint={blueprint.data}
                    contextId={selectedContextId}
                    contexts={contexts.data}
                    defaultContextId={defaultContextId}
                    entityId={entityId}
                    // A cached form must not become the save baseline before
                    // the opening refresh has completed.
                    form={
                      entityForm.isFetchedAfterMount
                        ? entityForm.data
                        : undefined
                    }
                    statusParentContextIds={statusParentContexts(
                      contexts.data,
                      contextId,
                    )}
                    statusTransitions={statusTransitions.data}
                    inlineFieldsRef={inlineFieldsRef}
                    onContextChange={setSelectedContext}
                    resolved={resolved.data}
                    singleColumn={compact}
                  />
                )}
              </>
            )}
            {resolved.data && (
              <RecordControlsPanel
                attributes={blueprint.data?.attributes ?? []}
                entityId={entityId}
              />
            )}
            {showComments && resolved.data && (
              <Suspense
                fallback={
                  <CircularProgress aria-label={t('comments.loading')} />
                }
              >
                <EntityCommentsPanel key={entityId} entityId={entityId} />
              </Suspense>
            )}
          </>
        ),
      })}
      {duplicateOpen && (
        <DuplicateEntityDialog
          entityId={entityId}
          onClose={() => setDuplicateOpen(false)}
          onDuplicated={(copy) => {
            setDuplicateOpen(false);
            onDuplicated(copy);
          }}
        />
      )}
      {deleteOpen && (
        <DeleteEntityDialog
          entityId={entityId}
          onClose={() => setDeleteOpen(false)}
          onDeleted={() => {
            setDeleteOpen(false);
            onDeleted();
          }}
        />
      )}
      <EntityAgentDrawer
        key={`${entityId}:${selectedContextId ?? ''}`}
        entityId={entityId}
        contextId={selectedContextId}
        offset={drawerOffset}
        onClose={() => setAgentPanelOpen(false)}
        open={agentPanelOpen}
        // Suggestions apply to the fields only when this user can edit them.
        draft={
          entityForm.data?.can_write && contextId
            ? {
                entityId,
                contextId,
                defaultContextId,
                getDraftValues: () =>
                  inlineFieldsRef.current?.getDraftValues() ?? {},
                onApply: (fields) =>
                  inlineFieldsRef.current?.applySmartFillValues(fields),
              }
            : undefined
        }
      />
      <EntityExtensionDrawer
        blueprintId={resolvedEntity?.blueprint_id ?? ''}
        blueprintVersion={
          resolvedEntity?.blueprint_version ?? FALLBACK_BLUEPRINT_VERSION
        }
        contextId={selectedContextId}
        entityId={entityId}
        offset={drawerOffset}
        onClose={() => setExtensionPanelOpen(false)}
        open={extensionPanelOpen}
        showContent={Boolean(loaded)}
      />
    </>
  );
};

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Box, CircularProgress } from '@mui/material';
import { lazy, Suspense, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { DataQualityFindingsChip } from './components/DataQualityFindingsChip';
import { EntityBlueprintHeaderActions } from './components/EntityBlueprintHeaderActions';
import { EntityHeading } from './components/EntityHeading';
import { EntityPageEyebrow } from './components/EntityPageEyebrow';
import { EntityPreviewContent } from './components/EntityPreviewContent';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityExtensionDrawer } from './components/EntityExtensionDrawer';
import { EntityAgentDrawer } from './components/EntityAgentDrawer';
import { EntityPreviewToolbar } from './components/EntityPreviewToolbar';
import { DeleteEntityDialog } from './components/DeleteEntityDialog';
import { RelationshipPickerActionBar } from './components/RelationshipPickerActionBar';
import { RecordControlsPanel } from './components/RecordControlsPanel';
import { useEntityPublications } from './components/useEntityPublications';
import { ExtensionOutlet } from '../extensions/ExtensionOutlet';
import {
  duplicateEntity,
  getBlueprintRevision,
  getCurrentBlueprint,
} from './api';
import {
  ENTITY_HEADER_CONTEXT_VERSION,
  FALLBACK_BLUEPRINT_VERSION,
} from './constants';
import { entityQueryKeys } from './queryKeys';
import { invalidateEntitySearches } from './invalidateEntity';
import {
  useEntityContextSelection,
  useResolvedEntityPreview,
} from './useEntityContexts';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { lexiconText } from '../lexicon/lexicon';

const EntityCommentsPanel = lazy(() =>
  import('../entity-comments/EntityCommentsPanel').then((module) => ({
    default: module.EntityCommentsPanel,
  })),
);

export const EntityPreviewPage = ({
  entityId,
  relationshipPickerToken,
}: {
  entityId: string;
  relationshipPickerToken?: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const navigate = useNavigate();
  const [deleteOpen, setDeleteOpen] = useState(false);
  const [extensionPanelOpen, setExtensionPanelOpen] = useState(false);
  const [agentPanelOpen, setAgentPanelOpen] = useState(false);
  const { contextId, contexts, setSelectedContext } =
    useEntityContextSelection();
  const selectedContextId = contextId ?? undefined;
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const publications = useEntityPublications(entityId, contextId);
  const duplicate = useMutation({
    mutationFn: () => duplicateEntity(entityId),
    onSuccess: (entity) => {
      void invalidateEntitySearches(client);
      void navigate({
        params: { entityId: entity.id },
        to: '/entities/$entityId/edit',
      });
    },
  });
  const resolved = useResolvedEntityPreview(entityId, contextId);
  const resolvedEntity = resolved.data?.entity;
  const blueprint = useQuery({
    queryKey: entityQueryKeys.blueprintRevision(
      resolvedEntity?.blueprint_id,
      resolvedEntity?.blueprint_version,
    ),
    queryFn: () => {
      if (!resolvedEntity)
        throw new Error(t('entities.entityPreviewUnavailable'));
      return getBlueprintRevision(
        resolvedEntity.blueprint_id,
        resolvedEntity.blueprint_version,
      );
    },
    enabled: Boolean(
      resolvedEntity?.blueprint_id && resolvedEntity.blueprint_version,
    ),
  });
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
  const actionError = publications.error ?? duplicate.error;
  const loaded = resolved.data && blueprint.data;

  return (
    <PageContainer>
      <PageHeader
        actions={
          <EntityBlueprintHeaderActions
            blueprint={blueprint.data?.blueprint}
            isSample={resolvedEntity?.is_sample}
          >
            <DataQualityFindingsChip entityId={entityId} />
          </EntityBlueprintHeaderActions>
        }
        eyebrow={
          <EntityPageEyebrow
            blueprint={blueprint.data?.blueprint}
            label={t('entities.entityPreview')}
          />
        }
      />
      {relationshipPickerToken && (
        <RelationshipPickerActionBar
          entityId={entityId}
          pickerToken={relationshipPickerToken}
        />
      )}
      {resolved.data && blueprint.data && (
        <EntityHeading
          attributes={blueprint.data.attributes}
          entityId={entityId}
          values={resolved.data.values}
          view={blueprint.data.blueprint.views.detail}
        />
      )}
      {actionError && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {actionError.message}
        </Alert>
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
        onDuplicate={() => duplicate.mutate()}
        duplicatePending={duplicate.isPending}
        canDelete={
          session.data?.capabilities?.entities_delete === true &&
          Boolean(resolved.data)
        }
        onDelete={() => setDeleteOpen(true)}
        publication={publications.publication}
        canPublish={session.data?.capabilities?.entities_publish === true}
        onPublish={publications.publish}
        onPublishAll={publications.publishAll}
        onUnpublish={publications.unpublish}
        publicationPending={publications.isPending}
      />
      {deleteOpen && (
        <DeleteEntityDialog
          entityId={entityId}
          onClose={() => setDeleteOpen(false)}
          onDeleted={() => {
            setDeleteOpen(false);
            void navigate({ to: '/' });
          }}
        />
      )}
      <EntitySchemaSubheader
        entityId={entityId}
        name={blueprint.data && lexiconText(blueprint.data.blueprint.name)}
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
            <Box sx={{ display: 'flex', justifyContent: 'center', mt: 3 }}>
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
              contextsPending={contexts.isPending}
              entityId={entityId}
              onContextChange={setSelectedContext}
              resolved={resolved.data}
            />
          )}
        </>
      )}
      {resolved.data && <RecordControlsPanel entityId={entityId} />}
      {resolved.data && (
        <Suspense
          fallback={<CircularProgress aria-label={t('comments.loading')} />}
        >
          <EntityCommentsPanel key={entityId} entityId={entityId} />
        </Suspense>
      )}
      <EntityAgentDrawer
        key={`${entityId}:${selectedContextId ?? ''}`}
        entityId={entityId}
        contextId={selectedContextId}
        onClose={() => setAgentPanelOpen(false)}
        open={agentPanelOpen}
      />
      <EntityExtensionDrawer
        blueprintId={resolvedEntity?.blueprint_id ?? ''}
        blueprintVersion={
          resolvedEntity?.blueprint_version ?? FALLBACK_BLUEPRINT_VERSION
        }
        contextId={selectedContextId}
        entityId={entityId}
        onClose={() => setExtensionPanelOpen(false)}
        open={extensionPanelOpen}
        showContent={Boolean(loaded)}
      />
    </PageContainer>
  );
};

import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { BlueprintIcon } from '../../components/systemIcons';
import {
  Alert,
  Box,
  Chip,
  CircularProgress,
  Paper,
  Tooltip,
  Typography,
} from '@mui/material';
import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RouterButton } from '../../components/RouterLink';
import { EntityContextPicker } from './components/EntityContextPicker';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityExtensionDrawer } from './components/EntityExtensionDrawer';
import { EntityAgentDrawer } from './components/EntityAgentDrawer';
import { EntityPreviewToolbar } from './components/EntityPreviewToolbar';
import { DeleteEntityDialog } from './components/DeleteEntityDialog';
import {
  ExtensionOutlet,
  ExtensionPopoverOutlet,
} from '../extensions/ExtensionOutlet';
import { getExtensionRuntime } from '../extensions/api';
import { extensionQueryKeys } from '../extensions/queryKeys';
import { extensionRuntimeRefetchInterval } from '../extensions/constants';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/queryKeys';
import { defaultContextCode } from '../contexts/constants';
import { listFindings } from '../rules/api';
import { ruleQueryKeys } from '../rules/queryKeys';
import {
  duplicateEntity,
  getBlueprintRevision,
  getCurrentBlueprint,
  getResolvedEntityPreview,
  getEntityPublications,
  publishEntity,
  publishEntityAllChannels,
  unpublishEntity,
} from './api';
import { attributeLabel } from './entityDisplay';
import { entityQueryKeys } from './queryKeys';
import { EntityView } from '../views/components/EntityView';
import { RelationshipPickerActionBar } from './components/RelationshipPickerActionBar';
import {
  entityHeadingComponentId,
  findEntityHeading,
} from '../views/components/blocks/EntityHeadingDefinition';
import { resolveHeadingRenderer } from '../views/components/registry';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
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
  const [selectedContext, setSelectedContext] = useState('');
  const [deleteOpen, setDeleteOpen] = useState(false);
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const selectedContextId =
    selectedContext ||
    contexts.data?.find((context) => context.code === defaultContextCode)?.id;
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const publications = useQuery({
    queryKey: entityQueryKeys.publication(entityId),
    queryFn: () => getEntityPublications(entityId),
  });
  const invalidatePublications = () =>
    client.invalidateQueries({
      queryKey: entityQueryKeys.publication(entityId),
    });
  const publish = useMutation({
    mutationFn: (contextId: string) => publishEntity(entityId, contextId),
    onSuccess: invalidatePublications,
  });
  const publishAll = useMutation({
    mutationFn: () => publishEntityAllChannels(entityId),
    onSuccess: invalidatePublications,
  });
  const unpublish = useMutation({
    mutationFn: (contextId: string) => unpublishEntity(entityId, contextId),
    onSuccess: invalidatePublications,
  });
  const duplicate = useMutation({
    mutationFn: () => duplicateEntity(entityId),
    onSuccess: (entity) => {
      void client.invalidateQueries({ queryKey: entityQueryKeys.searches() });
      void navigate({
        params: { entityId: entity.id },
        to: '/entities/$entityId/edit',
      });
    },
  });
  const publication = publications.data?.find(
    (item) => item.context_id === selectedContextId,
  );
  const resolved = useQuery({
    queryKey: entityQueryKeys.resolvedPreview(entityId, selectedContextId),
    queryFn: () => {
      if (!selectedContextId) throw new Error('Preview context is unavailable');
      return getResolvedEntityPreview(entityId, selectedContextId);
    },
    enabled: Boolean(selectedContextId),
  });
  const blueprint = useQuery({
    queryKey: entityQueryKeys.blueprintRevision(
      resolved.data?.entity.blueprint_id,
      resolved.data?.entity.blueprint_version,
    ),
    queryFn: () => {
      if (!resolved.data) throw new Error('Entity preview is unavailable');
      return getBlueprintRevision(
        resolved.data.entity.blueprint_id,
        resolved.data.entity.blueprint_version,
      );
    },
    enabled: Boolean(
      resolved.data?.entity.blueprint_id &&
      resolved.data.entity.blueprint_version,
    ),
  });
  const attributePanels = useQuery({
    queryKey: extensionQueryKeys.runtime(
      blueprint.data
        ? {
            blueprintId: blueprint.data.blueprint.id,
            blueprintVersion: blueprint.data.blueprint.version,
          }
        : undefined,
    ),
    queryFn: () => {
      if (!blueprint.data) throw new Error('Blueprint revision is unavailable');
      return getExtensionRuntime({
        blueprintId: blueprint.data.blueprint.id,
        blueprintVersion: blueprint.data.blueprint.version,
      });
    },
    enabled: Boolean(blueprint.data),
    refetchInterval: extensionRuntimeRefetchInterval,
    retry: false,
  });
  const hasAttributePanels = attributePanels.data?.some(
    (item) => item.outlet === 'entity_attribute_panel' && item.kind === 'panel',
  );
  const hasFilePanels = attributePanels.data?.some(
    (item) => item.outlet === 'file_panel' && item.kind === 'panel',
  );
  const currentBlueprint = useQuery({
    queryKey: entityQueryKeys.currentBlueprint(
      resolved.data?.entity.blueprint_id ?? '',
    ),
    queryFn: () => {
      if (!resolved.data) throw new Error('Entity preview is unavailable');
      return getCurrentBlueprint(resolved.data.entity.blueprint_id);
    },
    enabled: Boolean(resolved.data?.entity.blueprint_id),
  });
  const findings = useQuery({
    queryKey: ruleQueryKeys.findings(entityId),
    queryFn: () => listFindings(entityId),
  });
  const detailView = blueprint.data?.blueprint.views.detail;
  const heading = findEntityHeading(detailView);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  const [extensionPanelOpen, setExtensionPanelOpen] = useState(false);
  const [agentPanelOpen, setAgentPanelOpen] = useState(false);
  const schemaOutdated =
    currentBlueprint.data && resolved.data
      ? currentBlueprint.data.blueprint.version >
        resolved.data.entity.blueprint_version
      : undefined;
  return (
    <PageContainer>
      <PageHeader
        actions={
          <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
            {findings.data?.some((finding) => finding.state !== 'resolved') && (
              <Chip
                color="warning"
                label={`${findings.data.filter((finding) => finding.state !== 'resolved').length} data quality finding(s)`}
                size="small"
              />
            )}
            {resolved.data?.entity.is_sample && (
              <Chip color="info" label={t('entities.sample')} size="small" />
            )}
            {blueprint.data && (
              <Tooltip title={blueprint.data.blueprint.name}>
                <RouterButton
                  params={{ blueprintId: blueprint.data.blueprint.id }}
                  size="small"
                  startIcon={<BlueprintIcon />}
                  to="/manage/blueprints/$blueprintId"
                  variant="text"
                >
                  {t('entities.blueprint')}: {blueprint.data.blueprint.name}
                </RouterButton>
              </Tooltip>
            )}
          </Box>
        }
        eyebrow={
          blueprint.data ? (
            <>
              {t('entities.entityPreview')} ·{' '}
              <Link
                search={{
                  blueprint: blueprint.data.blueprint.code,
                  version: blueprint.data.blueprint.version,
                }}
                to="/"
              >
                {t('entities.viewAll')}
              </Link>
            </>
          ) : (
            t('entities.entityPreview')
          )
        }
      />
      {relationshipPickerToken && (
        <RelationshipPickerActionBar
          entityId={entityId}
          pickerToken={relationshipPickerToken}
        />
      )}
      {resolved.data && blueprint.data && HeadingRenderer
        ? createElement(HeadingRenderer, {
            attributes: blueprint.data.attributes,
            entityId,
            values: resolved.data.values,
            view: detailView,
          })
        : null}
      {(publish.isError ||
        publishAll.isError ||
        unpublish.isError ||
        duplicate.isError) && (
        <Alert severity="error" sx={{ mt: 3 }}>
          {publish.error?.message ??
            publishAll.error?.message ??
            unpublish.error?.message ??
            duplicate.error?.message}
        </Alert>
      )}
      {resolved.data && blueprint.data && (
        <ExtensionOutlet
          context={{
            context_version: 1,
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
        showExtensions={Boolean(resolved.data && blueprint.data)}
        onDuplicate={() => duplicate.mutate()}
        duplicatePending={duplicate.isPending}
        canDelete={
          session.data?.capabilities?.entities_delete === true &&
          Boolean(resolved.data)
        }
        onDelete={() => setDeleteOpen(true)}
        publication={publication}
        canPublish={session.data?.capabilities?.entities_publish === true}
        onPublish={() => selectedContextId && publish.mutate(selectedContextId)}
        onPublishAll={() => publishAll.mutate()}
        onUnpublish={() =>
          selectedContextId && unpublish.mutate(selectedContextId)
        }
        publicationPending={
          publish.isPending || publishAll.isPending || unpublish.isPending
        }
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
        name={blueprint.data?.blueprint.name}
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
                    contexts={contexts.data}
                    disabled={contexts.isPending}
                    onChange={setSelectedContext}
                    value={selectedContextId ?? ''}
                  />
                  <EntityView
                    attributes={blueprint.data.attributes}
                    fallbackVisibilityScope="detail"
                    contextId={selectedContextId}
                    entityId={entityId}
                    renderAttributeDecoration={(attribute) => (
                      <ExtensionPopoverOutlet
                        context={{
                          attribute_id: attribute.id,
                          blueprint_id: blueprint.data.blueprint.id,
                          blueprint_version: blueprint.data.blueprint.version,
                          context_id: selectedContextId,
                          entity_id: entityId,
                        }}
                        key={String(attribute.id)}
                        label={t('entities.viewExtensionContent', {
                          attribute: attributeLabel(attribute),
                        })}
                        outlet="entity_attribute_decoration"
                        runtimeScope={{
                          blueprintId: blueprint.data.blueprint.id,
                          blueprintVersion: blueprint.data.blueprint.version,
                        }}
                      />
                    )}
                    renderAttributePanel={
                      hasAttributePanels
                        ? (attribute) => (
                            <ExtensionOutlet
                              context={{
                                context_version: 1,
                                entity_id: entityId,
                                attribute_id: attribute.id,
                                blueprint_id: blueprint.data.blueprint.id,
                                blueprint_version:
                                  blueprint.data.blueprint.version,
                                context_id: selectedContextId ?? null,
                              }}
                              outlet="entity_attribute_panel"
                              runtimeScope={{
                                blueprintId: blueprint.data.blueprint.id,
                                blueprintVersion:
                                  blueprint.data.blueprint.version,
                              }}
                            />
                          )
                        : undefined
                    }
                    renderFilePanel={
                      hasFilePanels
                        ? (attribute, fileId) => (
                            <ExtensionOutlet
                              context={{
                                context_version: 1,
                                file_id: fileId,
                                entity_id: entityId,
                                attribute_id: attribute.id,
                                blueprint_id: blueprint.data.blueprint.id,
                                blueprint_version:
                                  blueprint.data.blueprint.version,
                              }}
                              outlet="file_panel"
                              runtimeScope={{
                                blueprintId: blueprint.data.blueprint.id,
                                blueprintVersion:
                                  blueprint.data.blueprint.version,
                              }}
                            />
                          )
                        : undefined
                    }
                    values={resolved.data.values}
                    view={detailView}
                    skipComponentId={entityHeadingComponentId}
                  />
                  {(resolved.data.reusable_attributes?.length ?? 0) > 0 && (
                    <Box component="section" sx={{ mt: 4 }}>
                      <Typography component="h2" variant="h6">
                        {t('entities.additionalAttributes')}
                      </Typography>
                      <EntityView
                        attributes={resolved.data.reusable_attributes}
                        contextId={selectedContextId}
                        entityId={entityId}
                        values={resolved.data.reusable_values ?? {}}
                      />
                    </Box>
                  )}
                </Paper>
                <ExtensionOutlet
                  context={{
                    entity_id: entityId,
                    context_id: selectedContextId,
                  }}
                  outlet="entity_action"
                  runtimeScope={{
                    blueprintId: blueprint.data.blueprint.id,
                    blueprintVersion: blueprint.data.blueprint.version,
                  }}
                />
              </Box>
            </Box>
          )}
        </>
      )}
      <EntityAgentDrawer
        key={`${entityId}:${selectedContextId ?? ''}`}
        entityId={entityId}
        contextId={selectedContextId}
        onClose={() => setAgentPanelOpen(false)}
        open={agentPanelOpen}
      />
      <EntityExtensionDrawer
        blueprintId={resolved.data?.entity.blueprint_id ?? ''}
        blueprintVersion={resolved.data?.entity.blueprint_version ?? 1}
        contextId={selectedContextId}
        entityId={entityId}
        onClose={() => setExtensionPanelOpen(false)}
        open={extensionPanelOpen}
        showContent={Boolean(resolved.data && blueprint.data)}
      />
    </PageContainer>
  );
};

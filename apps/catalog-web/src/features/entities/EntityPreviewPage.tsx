import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { BlueprintIcon } from '../../components/system-icons';
import { Alert, Box, Paper, Tooltip, Typography } from '@mui/material';
import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { QueryErrorNotice } from '../../components/QueryErrorNotice';
import { RouterButton } from '../../components/RouterLink';
import { EntityContextPicker } from './components/EntityContextPicker';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityExtensionDrawer } from './components/EntityExtensionDrawer';
import { EntityPreviewToolbar } from './components/EntityPreviewToolbar';
import {
  ExtensionOutlet,
  ExtensionPopoverOutlet,
} from '../extensions/ExtensionOutlet';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/query-keys';
import { defaultContextCode } from '../contexts/constants';
import {
  getBlueprintRevision,
  getCurrentBlueprint,
  getResolvedEntityPreview,
} from './api';
import { attributeLabel } from './entity-display';
import { entityQueryKeys } from './query-keys';
import { EntityView } from '../views/components/EntityView';
import { RelationshipPickerActionBar } from './components/RelationshipPickerActionBar';
import {
  entityHeadingComponentId,
  findEntityHeading,
} from '../views/components/blocks/EntityHeadingDefinition';
import { resolveHeadingRenderer } from '../views/components/registry';
export const EntityPreviewPage = ({
  entityId,
  relationshipPickerToken,
}: {
  entityId: string;
  relationshipPickerToken?: string;
}) => {
  const { t } = useTranslation();
  const [selectedContext, setSelectedContext] = useState('');
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const selectedContextId =
    selectedContext ||
    contexts.data?.find((context) => context.code === defaultContextCode)?.id;
  const resolved = useQuery({
    queryKey: entityQueryKeys.resolvedPreview(entityId, selectedContextId),
    queryFn: () => getResolvedEntityPreview(entityId, selectedContextId!),
    enabled: Boolean(selectedContextId),
  });
  const blueprint = useQuery({
    queryKey: entityQueryKeys.blueprintRevision(
      resolved.data?.entity.blueprint_id,
      resolved.data?.entity.blueprint_version,
    ),
    queryFn: () =>
      getBlueprintRevision(
        resolved.data!.entity.blueprint_id!,
        resolved.data!.entity.blueprint_version!,
      ),
    enabled: Boolean(
      resolved.data?.entity.blueprint_id &&
      resolved.data.entity.blueprint_version,
    ),
  });
  const currentBlueprint = useQuery({
    queryKey: entityQueryKeys.currentBlueprint(
      resolved.data?.entity.blueprint_id ?? '',
    ),
    queryFn: () => getCurrentBlueprint(resolved.data!.entity.blueprint_id!),
    enabled: Boolean(resolved.data?.entity.blueprint_id),
  });
  const detailView = blueprint.data?.blueprint.views.detail;
  const heading = findEntityHeading(detailView);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  const [extensionPanelOpen, setExtensionPanelOpen] = useState(false);
  const schemaOutdated =
    currentBlueprint.data && resolved.data
      ? currentBlueprint.data.blueprint.version >
        (resolved.data.entity.blueprint_version ?? Infinity)
      : undefined;
  return (
    <PageContainer>
      <PageHeader
        actions={
          blueprint.data && (
            <Tooltip title={blueprint.data.blueprint.name}>
              <RouterButton
                params={{ blueprintId: String(blueprint.data.blueprint.id) }}
                size="small"
                startIcon={<BlueprintIcon />}
                to="/manage/blueprints/$blueprintId"
                variant="text"
              >
                {t('entities.blueprint')}: {blueprint.data.blueprint.name}
              </RouterButton>
            </Tooltip>
          )
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
      <EntityPreviewToolbar
        entityId={entityId}
        extensionPanelOpen={extensionPanelOpen}
        onOpenExtensions={() => setExtensionPanelOpen(true)}
        schemaOutdated={schemaOutdated}
        showExtensions={Boolean(resolved.data && blueprint.data)}
      />
      <EntitySchemaSubheader
        entityId={entityId}
        name={blueprint.data?.blueprint.name}
      />
      {contexts.isPending && (
        <Typography sx={{ py: 3 }}>{t('entities.loadingContexts')}</Typography>
      )}
      <QueryErrorNotice
        error={contexts.error}
        isRetrying={contexts.isFetching}
        onRetry={() => void contexts.refetch()}
      />
      {contexts.data && (
        <>
          {resolved.isPending && (
            <Typography sx={{ mt: 3 }}>
              {t('entities.resolvingValues')}
            </Typography>
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
                      />
                    )}
                    values={resolved.data.values}
                    view={detailView}
                    skipComponentId={entityHeadingComponentId}
                  />
                </Paper>
                <ExtensionOutlet
                  context={{
                    entity_id: entityId,
                    context_id: selectedContextId,
                  }}
                  outlet="entity_action"
                />
              </Box>
            </Box>
          )}
        </>
      )}
      <EntityExtensionDrawer
        contextId={selectedContextId}
        entityId={entityId}
        onClose={() => setExtensionPanelOpen(false)}
        open={extensionPanelOpen}
        showContent={Boolean(resolved.data && blueprint.data)}
      />
    </PageContainer>
  );
};

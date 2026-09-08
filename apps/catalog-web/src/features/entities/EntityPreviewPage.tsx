import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import CategoryOutlinedIcon from '@mui/icons-material/CategoryOutlined';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import CloseIcon from '@mui/icons-material/Close';
import EditOutlinedIcon from '@mui/icons-material/EditOutlined';
import HistoryOutlinedIcon from '@mui/icons-material/HistoryOutlined';
import UpgradeOutlinedIcon from '@mui/icons-material/UpgradeOutlined';
import ViewSidebarOutlinedIcon from '@mui/icons-material/ViewSidebarOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import {
  Alert,
  Box,
  Button,
  Drawer,
  IconButton,
  Paper,
  Tooltip,
  Typography,
} from '@mui/material';
import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { EntityContextPicker } from './components/EntityContextPicker';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityToolbar } from './components/EntityToolbar';
import {
  ExtensionOutlet,
  ExtensionPopoverOutlet,
} from '../extensions/ExtensionOutlet';
import {
  getBlueprintRevision,
  getCurrentBlueprint,
  getResolvedEntityPreview,
  listContexts,
} from './api';
import { entityQueryKeys } from './query-keys';
import { EntityView } from '../views/components/EntityView';
import {
  entityHeadingComponentId,
  findEntityHeading,
} from '../views/components/blocks/EntityHeadingDefinition';
import { resolveHeadingRenderer } from '../views/components/registry';
export const EntityPreviewPage = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const [selectedContext, setSelectedContext] = useState('');
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const selectedContextId =
    selectedContext ||
    contexts.data?.find((context) => context.code === 'default')?.id;
  const resolved = useQuery({
    queryKey: selectedContextId
      ? entityQueryKeys.resolvedPreview(entityId, selectedContextId)
      : ['entity-resolved-preview'],
    queryFn: () => getResolvedEntityPreview(entityId, selectedContextId!),
    enabled: Boolean(selectedContextId),
  });
  const blueprint = useQuery({
    queryKey: resolved.data
      ? entityQueryKeys.blueprintRevision(
          resolved.data.entity.blueprint_id!,
          resolved.data.entity.blueprint_version!,
        )
      : ['blueprint-revision'],
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
  return (
    <PageContainer>
      <PageHeader
        actions={
          blueprint.data && (
            <Tooltip title={blueprint.data.blueprint.name}>
              <Button
                component={Link}
                params={{ blueprintId: blueprint.data.blueprint.id }}
                size="small"
                startIcon={<CategoryOutlinedIcon />}
                to="/manage/blueprints/$blueprintId"
                variant="text"
              >
                {t('entities.blueprint')}: {blueprint.data.blueprint.name}
              </Button>
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
      {resolved.data && blueprint.data && HeadingRenderer
        ? createElement(HeadingRenderer, {
            attributes: blueprint.data.attributes,
            entityId,
            values: resolved.data.values,
            view: detailView,
          })
        : null}
      <EntityToolbar label={t('entities.entityPreview')}>
        <Tooltip title={t('entities.editEntity')}>
          <IconButton
            aria-label={t('entities.editEntity')}
            component={Link}
            params={{ entityId }}
            to="/entities/$entityId/edit"
          >
            <EditOutlinedIcon />
          </IconButton>
        </Tooltip>
        <Tooltip title={t('entities.changes')}>
          <IconButton
            aria-label={t('entities.changes')}
            component={Link}
            params={{ entityId }}
            to="/entities/$entityId/changes"
          >
            <HistoryOutlinedIcon />
          </IconButton>
        </Tooltip>
        {currentBlueprint.data && resolved.data && (
          <>
            {currentBlueprint.data.blueprint.version >
            (resolved.data.entity.blueprint_version ?? Infinity) ? (
              <>
                <Tooltip title={t('entities.schemaOutdated')}>
                  <WarningAmberOutlinedIcon color="warning" fontSize="small" />
                </Tooltip>
                <Tooltip title={t('entities.upgradeBlueprint')}>
                  <IconButton
                    aria-label={t('entities.upgradeBlueprint')}
                    component={Link}
                    params={{ entityId }}
                    to="/entities/$entityId/migrate"
                  >
                    <UpgradeOutlinedIcon />
                  </IconButton>
                </Tooltip>
              </>
            ) : (
              <Tooltip title={t('entities.matchesCurrentSchema')}>
                <CheckCircleOutlinedIcon color="success" fontSize="small" />
              </Tooltip>
            )}
          </>
        )}
        <Box sx={{ flexGrow: 1 }} />
        {resolved.data && blueprint.data && (
          <Tooltip title={t('entities.extensionContributions')}>
            <IconButton
              aria-controls={
                extensionPanelOpen
                  ? 'entity-extension-contributions'
                  : undefined
              }
              aria-expanded={extensionPanelOpen}
              aria-label={t('entities.extensionContributions')}
              onClick={() => setExtensionPanelOpen(true)}
            >
              <ViewSidebarOutlinedIcon />
            </IconButton>
          </Tooltip>
        )}
      </EntityToolbar>
      <EntitySchemaSubheader
        entityId={entityId}
        name={blueprint.data?.blueprint.name}
      />
      {contexts.isPending && (
        <Typography sx={{ py: 3 }}>{t('entities.loadingContexts')}</Typography>
      )}
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
                        key={attribute.id}
                        label={t('entities.viewExtensionContent', {
                          attribute: attribute.code.replaceAll('_', ' '),
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
      <Drawer
        anchor="right"
        onClose={() => setExtensionPanelOpen(false)}
        open={extensionPanelOpen}
        variant="persistent"
      >
        <Box
          id="entity-extension-contributions"
          sx={{ p: 3, width: { xs: '100vw', sm: 480 } }}
        >
          <Box sx={{ alignItems: 'center', display: 'flex' }}>
            <Typography sx={{ flexGrow: 1 }} variant="h6">
              {t('entities.extensionContributions')}
            </Typography>
            <IconButton
              aria-label={t('entities.closeExtensionContributions')}
              onClick={() => setExtensionPanelOpen(false)}
            >
              <CloseIcon />
            </IconButton>
          </Box>
          {resolved.data && blueprint.data && (
            <Box sx={{ mt: 2 }}>
              <ExtensionOutlet
                context={{
                  entity_id: entityId,
                  context_id: selectedContextId,
                }}
                outlet="entity_preview_panel"
              />
            </Box>
          )}
        </Box>
      </Drawer>
    </PageContainer>
  );
};

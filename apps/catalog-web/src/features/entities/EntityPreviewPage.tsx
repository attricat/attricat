import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import {
  Alert,
  Box,
  MenuItem,
  Paper,
  TextField,
  Typography,
} from '@mui/material';
import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
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
  const [selectedContext, setSelectedContext] = useState('default');
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const selectedContextId = contexts.data?.find(
    (context) => context.code === selectedContext,
  )?.id;
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
  return (
    <PageContainer maxWidth="lg">
      <PageHeader eyebrow={t('entities.entityPreview')} />
      {resolved.data && blueprint.data && HeadingRenderer
        ? createElement(HeadingRenderer, {
            attributes: blueprint.data.attributes,
            entityId,
            values: resolved.data.values,
            view: detailView,
          })
        : null}
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId/edit">
          {t('entities.editEntity')}
        </Link>
        {' | '}
        <Link params={{ entityId }} to="/entities/$entityId/changes">
          {t('entities.changes')}
        </Link>
        {currentBlueprint.data && resolved.data && (
          <>
            {' | '}
            {currentBlueprint.data.blueprint.version >
            (resolved.data.entity.blueprint_version ?? Infinity) ? (
              <>
                <Typography
                  color="warning.main"
                  component="span"
                  sx={{
                    display: 'inline-flex',
                    gap: 0.5,
                    verticalAlign: 'middle',
                  }}
                >
                  <WarningAmberOutlinedIcon fontSize="small" />
                  {t('entities.schemaOutdated')}
                </Typography>
                {' | '}
                <Link params={{ entityId }} to="/entities/$entityId/migrate">
                  {t('entities.upgradeBlueprint')}
                </Link>
              </>
            ) : (
              <Typography
                component="span"
                sx={{
                  display: 'inline-flex',
                  gap: 0.5,
                  verticalAlign: 'middle',
                }}
              >
                <CheckCircleOutlinedIcon color="success" fontSize="small" />
                {t('entities.matchesCurrentSchema')}
              </Typography>
            )}
          </>
        )}
      </Box>
      {contexts.isPending && (
        <Typography sx={{ py: 3 }}>{t('entities.loadingContexts')}</Typography>
      )}
      {contexts.data && (
        <>
          <TextField
            select
            fullWidth
            label={t('entities.context')}
            onChange={(event) => setSelectedContext(event.target.value)}
            sx={{ mt: 3 }}
            value={selectedContext}
          >
            {(contexts.data ?? []).map((context) => (
              <MenuItem key={context.id} value={context.code}>
                {context.code === 'default'
                  ? t('entities.default')
                  : context.code}
              </MenuItem>
            ))}
          </TextField>
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
                gridTemplateColumns: {
                  xs: 'minmax(0, 1fr)',
                  md: 'minmax(0, 2fr) minmax(280px, 1fr)',
                },
                mt: 3,
              }}
            >
              <Box>
                <Paper component="section" sx={{ p: { xs: 2, md: 3 } }}>
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
              <Box>
                <ExtensionOutlet
                  context={{
                    entity_id: entityId,
                    context_id: selectedContextId,
                  }}
                  outlet="entity_preview_panel"
                />
              </Box>
            </Box>
          )}
        </>
      )}
    </PageContainer>
  );
};

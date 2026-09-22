import { useMutation, useQuery } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { BlueprintIcon } from '../../components/system-icons';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import UpgradeOutlinedIcon from '@mui/icons-material/UpgradeOutlined';
import ViewListOutlinedIcon from '@mui/icons-material/ViewListOutlined';
import VisibilityOutlinedIcon from '@mui/icons-material/VisibilityOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import { Alert, Box, Chip, Tooltip, Typography } from '@mui/material';
import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listContexts } from '../contexts/api';
import { contextQueryKeys } from '../contexts/query-keys';
import { defaultContextCode } from '../contexts/constants';
import {
  getEntityForm,
  getCurrentBlueprint,
  getResolvedEntityPreview,
  updateEntity,
} from './api';
import { RouterButton, RouterIconButton } from '../../components/RouterLink';
import { EntityContextPicker } from './components/EntityContextPicker';
import { EntityForm } from './components/EntityForm';
import { EntitySchemaSubheader } from './components/EntitySchemaSubheader';
import { EntityToolbar } from './components/EntityToolbar';
import { PageContainer } from '../../components/PageContainer';
import { PageHeader } from '../../components/PageHeader';
import { valuesForForm } from './entity-form';
import { entityQueryKeys } from './query-keys';
import { findEntityHeading } from '../views/components/blocks/EntityHeadingDefinition';
import { resolveHeadingRenderer } from '../views/components/registry';

export const EditEntityPage = ({ entityId }: { entityId: string }) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/entities/$entityId/edit' });
  const [selectedContext, setSelectedContext] = useState('');
  const entityForm = useQuery({
    queryKey: entityQueryKeys.form(entityId),
    queryFn: () => getEntityForm(entityId),
  });
  const update = useMutation({
    mutationFn: (input: Parameters<typeof updateEntity>[1]) =>
      updateEntity(entityId, input),
    onSuccess: (entity) => {
      void navigate({
        to: '/entities/$entityId',
        params: { entityId: entity.id },
      });
    },
  });
  const blueprintId = entityForm.data?.entity.blueprint_id;
  const currentBlueprint = useQuery({
    queryKey: entityQueryKeys.currentBlueprint(blueprintId ?? ''),
    queryFn: () => getCurrentBlueprint(blueprintId!),
    enabled: Boolean(blueprintId),
  });
  const contexts = useQuery({
    queryKey: contextQueryKeys.all(),
    queryFn: ({ signal }) => listContexts(signal),
  });
  const contextId =
    selectedContext ||
    (contexts.data?.find((context) => context.code === defaultContextCode)
      ?.id ??
      null);
  const defaultContextId =
    contexts.data?.find((context) => context.code === defaultContextCode)?.id ??
    null;
  const resolvedPreview = useQuery({
    queryKey: entityQueryKeys.resolvedPreview(entityId, contextId ?? undefined),
    queryFn: () => {
      if (!contextId) throw new Error('Preview context is unavailable');
      return getResolvedEntityPreview(entityId, contextId);
    },
    enabled: contextId !== null,
  });
  const detailView = entityForm.data?.blueprint.blueprint.views?.detail;
  const heading = findEntityHeading(detailView);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  return (
    <PageContainer>
      <PageHeader
        actions={
          entityForm.data && (
            <Box sx={{ alignItems: 'center', display: 'flex', gap: 1 }}>
              {entityForm.data.entity.is_sample && (
                <Chip color="info" label={t('entities.sample')} size="small" />
              )}
              <Tooltip title={entityForm.data.blueprint.blueprint.name}>
                <RouterButton
                  params={{ blueprintId: entityForm.data.entity.blueprint_id }}
                  size="small"
                  startIcon={<BlueprintIcon />}
                  to="/manage/blueprints/$blueprintId"
                  variant="text"
                >
                  {t('entities.blueprint')}:{' '}
                  {entityForm.data.blueprint.blueprint.name}
                </RouterButton>
              </Tooltip>
            </Box>
          )
        }
        eyebrow={
          entityForm.data ? (
            <>
              {t('entities.editEntity')} ·{' '}
              <Link
                search={{
                  blueprint: entityForm.data.blueprint.blueprint.code,
                  version: entityForm.data.blueprint.blueprint.version,
                }}
                to="/"
              >
                {t('entities.viewAll')}
              </Link>
            </>
          ) : (
            t('entities.editEntity')
          )
        }
      />
      {entityForm.data && resolvedPreview.data && HeadingRenderer
        ? createElement(HeadingRenderer, {
            attributes: entityForm.data.blueprint.attributes,
            entityId,
            values: resolvedPreview.data.values,
            view: detailView,
          })
        : null}
      <EntityToolbar label={t('entities.editEntity')}>
        <Tooltip title={t('entities.viewPreview')}>
          <RouterIconButton
            aria-label={t('entities.viewPreview')}
            params={{ entityId }}
            to="/entities/$entityId"
          >
            <VisibilityOutlinedIcon />
          </RouterIconButton>
        </Tooltip>
        {entityForm.data && (
          <Tooltip title={t('entities.viewAll')}>
            <RouterIconButton
              aria-label={t('entities.viewAll')}
              search={{
                blueprint: entityForm.data.blueprint.blueprint.code,
                version: entityForm.data.blueprint.blueprint.version,
              }}
              to="/"
            >
              <ViewListOutlinedIcon />
            </RouterIconButton>
          </Tooltip>
        )}
        {currentBlueprint.data &&
          entityForm.data &&
          (currentBlueprint.data.blueprint.version >
          (entityForm.data.entity.blueprint_version ?? Infinity) ? (
            <>
              <Tooltip title={t('entities.schemaOutdated')}>
                <WarningAmberOutlinedIcon color="warning" fontSize="small" />
              </Tooltip>
              <Tooltip title={t('entities.upgradeBlueprint')}>
                <RouterIconButton
                  aria-label={t('entities.upgradeBlueprint')}
                  params={{ entityId }}
                  to="/entities/$entityId/migrate"
                >
                  <UpgradeOutlinedIcon />
                </RouterIconButton>
              </Tooltip>
            </>
          ) : (
            <Tooltip title={t('entities.matchesCurrentSchema')}>
              <CheckCircleOutlinedIcon color="success" fontSize="small" />
            </Tooltip>
          ))}
      </EntityToolbar>
      <EntitySchemaSubheader
        entityId={entityId}
        name={entityForm.data?.blueprint.blueprint.name}
      />
      {entityForm.isPending && (
        <Typography sx={{ mt: 4 }}>{t('entities.loadingEntity')}</Typography>
      )}
      {(entityForm.error || update.error) && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {(entityForm.error ?? update.error)?.message}
        </Alert>
      )}
      {resolvedPreview.isError && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {resolvedPreview.error.message}
        </Alert>
      )}
      {entityForm.data && (
        <EntityForm
          key={`${entityForm.data.entity.id}:${contextId ?? ''}`}
          blueprint={entityForm.data.blueprint}
          contextId={contextId}
          contextPicker={
            <EntityContextPicker
              contexts={contexts.data ?? []}
              disabled={contexts.isPending}
              onChange={setSelectedContext}
              value={contextId ?? ''}
            />
          }
          entityId={entityId}
          defaultContextId={defaultContextId}
          existingValues={entityForm.data.values}
          resolvedValues={resolvedPreview.data?.values}
          showBlueprintMetadata={false}
          initialValues={valuesForForm(
            entityForm.data.blueprint.attributes,
            entityForm.data.values,
            contextId,
          )}
          isLoadingBlueprint={update.isPending}
          onSubmit={(input) => update.mutate(input)}
          submitLabel={t('entities.saveChanges')}
        />
      )}
    </PageContainer>
  );
};

import { useMutation, useQuery } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import CategoryOutlinedIcon from '@mui/icons-material/CategoryOutlined';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import {
  Alert,
  Box,
  Button,
  MenuItem,
  Paper,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { createElement, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getEntityForm,
  getCurrentBlueprint,
  getResolvedEntityPreview,
  listContexts,
  updateEntity,
} from './api';
import { EntityForm } from './components/EntityForm';
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
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const contextId =
    selectedContext ||
    (contexts.data?.find((context) => context.code === 'default')?.id ?? null);
  const defaultContextId =
    contexts.data?.find((context) => context.code === 'default')?.id ?? null;
  const resolvedPreview = useQuery({
    queryKey: contextId
      ? entityQueryKeys.resolvedPreview(entityId, contextId)
      : ['entity-resolved-preview'],
    queryFn: () => getResolvedEntityPreview(entityId, contextId!),
    enabled: contextId !== null,
  });
  const detailView = entityForm.data?.blueprint.views?.detail;
  const heading = findEntityHeading(detailView);
  const HeadingRenderer = resolveHeadingRenderer(heading?.component);
  return (
    <PageContainer>
      <PageHeader
        eyebrow={
          entityForm.data ? (
            <>
              {t('entities.editEntity')} ·{' '}
              <Tooltip title={entityForm.data.blueprint.blueprint.name}>
                <Link
                  params={{ blueprintId: entityForm.data.entity.blueprint_id! }}
                  to="/manage/blueprints/$blueprintId"
                >
                  <CategoryOutlinedIcon
                    fontSize="inherit"
                    sx={{ mr: 0.25, verticalAlign: 'text-bottom' }}
                  />
                  {t('entities.blueprint')}: {entityForm.data.blueprint.blueprint.name}
                </Link>
              </Tooltip>{' '}
              ·{' '}
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
      <Paper
        aria-label={t('entities.editEntity')}
        component="nav"
        sx={{
          alignItems: 'center',
          display: 'flex',
          flexWrap: 'wrap',
          gap: 1,
          mt: 3,
          p: 1.5,
        }}
      >
        <Button
          component={Link}
          params={{ entityId }}
          size="small"
          to="/entities/$entityId"
          variant="text"
        >
          {t('entities.viewPreview')}
        </Button>
        {entityForm.data && (
          <Tooltip title={entityForm.data.blueprint.blueprint.name}>
            <Button
              component={Link}
              params={{ blueprintId: entityForm.data.entity.blueprint_id! }}
              size="small"
              startIcon={<CategoryOutlinedIcon />}
              to="/manage/blueprints/$blueprintId"
              variant="text"
            >
              {t('entities.blueprint')}: {entityForm.data.blueprint.blueprint.name}
            </Button>
          </Tooltip>
        )}
        {entityForm.data && (
          <Button
            component={Link}
            search={{
              blueprint: entityForm.data.blueprint.blueprint.code,
              version: entityForm.data.blueprint.blueprint.version,
            }}
            size="small"
            to="/"
            variant="text"
          >
            {t('entities.viewAll')}
          </Button>
        )}
        {currentBlueprint.data &&
          entityForm.data &&
          (currentBlueprint.data.blueprint.version >
          (entityForm.data.entity.blueprint_version ?? Infinity) ? (
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
              <Button
                component={Link}
                params={{ entityId }}
                size="small"
                to="/entities/$entityId/migrate"
                variant="text"
              >
                {t('entities.upgradeBlueprint')}
              </Button>
            </>
          ) : (
            <Typography
              component="span"
              sx={{ display: 'inline-flex', gap: 0.5, verticalAlign: 'middle' }}
            >
              <CheckCircleOutlinedIcon color="success" fontSize="small" />
              {t('entities.matchesCurrentSchema')}
            </Typography>
          ))}
        {entityForm.data && (
          <TextField
            select
            disabled={contexts.isPending}
            label={t('entities.context')}
            onChange={(event) => setSelectedContext(event.target.value)}
            size="small"
            sx={{ maxWidth: '100%', width: 280 }}
            value={contextId ?? ''}
          >
            {(contexts.data ?? []).map((context) => (
              <MenuItem key={context.id} value={context.id}>
                {context.code}
              </MenuItem>
            ))}
          </TextField>
        )}
      </Paper>
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
          entityId={entityId}
          defaultContextId={defaultContextId}
          existingValues={entityForm.data.values}
          resolvedValues={resolvedPreview.data?.values}
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

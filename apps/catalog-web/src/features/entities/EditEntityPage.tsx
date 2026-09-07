import { useMutation, useQuery } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import CheckCircleOutlinedIcon from '@mui/icons-material/CheckCircleOutlined';
import WarningAmberOutlinedIcon from '@mui/icons-material/WarningAmberOutlined';
import { Alert, Box, MenuItem, TextField, Typography } from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  getEntityForm,
  getCurrentBlueprint,
  getResolvedEntityPreview,
  listContexts,
  updateEntity,
} from './api';
import { EntityForm } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { valuesForForm } from './entity-form';
import { entityQueryKeys } from './query-keys';

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
  return (
    <EntityPage title={t('entities.editEntity')}>
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId">
          {t('entities.viewPreview')}
        </Link>
        {' | '}
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
              {' | '}
              <Link params={{ entityId }} to="/entities/$entityId/migrate">
                {t('entities.upgradeBlueprint')}
              </Link>
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
      </Box>
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
        <>
          <TextField
            select
            fullWidth
            label={t('entities.context')}
            onChange={(event) => setSelectedContext(event.target.value)}
            sx={{ mt: 4 }}
            value={contextId ?? ''}
          >
            {(contexts.data ?? []).map((context) => (
              <MenuItem key={context.id} value={context.id}>
                {context.code}
              </MenuItem>
            ))}
          </TextField>
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
        </>
      )}
    </EntityPage>
  );
};

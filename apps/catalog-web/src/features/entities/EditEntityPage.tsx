import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogContent,
  DialogTitle,
  MenuItem,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import {
  getEntityForm,
  getResolvedEntityPreview,
  listContexts,
  migrateEntity,
  previewEntityMigration,
  updateEntity,
} from './api';
import { EntityForm } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { valuesForForm } from './entity-form';
import { entityQueryKeys } from './query-keys';

export const EditEntityPage = ({ entityId }: { entityId: string }) => {
  const navigate = useNavigate({ from: '/entities/$entityId/edit' });
  const queryClient = useQueryClient();
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
  const migrationPreview = useMutation({
    mutationFn: () => previewEntityMigration(entityId),
  });
  const migrate = useMutation({
    mutationFn: ({
      values,
      relationships,
    }: {
      values: Parameters<typeof migrateEntity>[1]['values'];
      relationships: Parameters<typeof migrateEntity>[1]['relationships'];
    }) => {
      if (!migrationPreview.data)
        throw new Error('Load the migration preview before upgrading');
      return migrateEntity(entityId, {
        migration_id: migrationPreview.data.migration_id,
        expected_target_version: migrationPreview.data.target.blueprint.version,
        values,
        relationships,
      });
    },
    onSuccess: () => {
      migrationPreview.reset();
      void queryClient.invalidateQueries({
        queryKey: entityQueryKeys.form(entityId),
      });
      void queryClient.invalidateQueries({
        queryKey: entityQueryKeys.resolvedPreview(entityId, contextId!),
      });
    },
  });
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const contextId =
    selectedContext ||
    (contexts.data?.find((context) => context.code === 'default')?.id ?? null);
  const resolvedPreview = useQuery({
    queryKey: contextId
      ? entityQueryKeys.resolvedPreview(entityId, contextId)
      : ['entity-resolved-preview'],
    queryFn: () => getResolvedEntityPreview(entityId, contextId!),
    enabled: contextId !== null,
  });
  return (
    <EntityPage title="Edit entity">
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId">
          View preview
        </Link>
      </Box>
      {entityForm.data && (
        <Button
          disabled={migrationPreview.isPending}
          onClick={() => migrationPreview.mutate()}
          sx={{ mt: 2 }}
          variant="outlined"
        >
          {migrationPreview.isPending
            ? 'Checking upgrade...'
            : 'Upgrade blueprint'}
        </Button>
      )}
      {entityForm.isPending && (
        <Typography sx={{ mt: 4 }}>Loading entity...</Typography>
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
      {migrationPreview.error && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {migrationPreview.error.message}
        </Alert>
      )}
      {entityForm.data && (
        <>
          <TextField
            select
            fullWidth
            label="Context"
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
            existingValues={entityForm.data.values}
            resolvedValues={resolvedPreview.data?.values}
            initialValues={valuesForForm(
              entityForm.data.blueprint.attributes,
              entityForm.data.values,
              contextId,
            )}
            isLoadingBlueprint={update.isPending}
            onSubmit={(input) => update.mutate(input)}
            submitLabel="Save changes"
          />
        </>
      )}
      <Dialog
        fullWidth
        maxWidth="md"
        onClose={() => migrationPreview.reset()}
        open={migrationPreview.data !== undefined}
      >
        <DialogTitle>
          Upgrade from v{migrationPreview.data?.source_version} to v
          {migrationPreview.data?.target.blueprint.version}
        </DialogTitle>
        <DialogContent>
          {migrationPreview.data?.issues.map((issue) => (
            <Alert
              key={`${issue.attribute_code}:${issue.message}`}
              severity="warning"
              sx={{ mt: 2 }}
            >
              {issue.attribute_code ? `${issue.attribute_code}: ` : ''}
              {issue.message}
            </Alert>
          ))}
          {migrationPreview.data?.status === 'blocked' && (
            <Alert severity="error" sx={{ mt: 2 }}>
              This entity cannot be upgraded until the incompatible values are
              resolved.
            </Alert>
          )}
          {migrationPreview.data &&
            migrationPreview.data.status !== 'blocked' && (
              <EntityForm
                blueprint={migrationPreview.data.target}
                contextId={contextId}
                error={migrate.error}
                existingValues={migrationPreview.data.values}
                initialValues={valuesForForm(
                  migrationPreview.data.target.attributes,
                  migrationPreview.data.values,
                  contextId,
                )}
                isLoadingBlueprint={migrate.isPending}
                onSubmit={({ values, relationships }) =>
                  migrate.mutate({ values, relationships })
                }
                submitLabel="Upgrade entity"
              />
            )}
        </DialogContent>
      </Dialog>
    </EntityPage>
  );
};

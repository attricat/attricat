import { useMutation, useQuery } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import { Alert, Box, MenuItem, TextField, Typography } from '@mui/material';
import { useState } from 'react';
import { getEntityForm, listContexts, updateEntity } from './api';
import { EntityForm } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { valuesForForm } from './entity-form';
import { entityQueryKeys } from './query-keys';

export const EditEntityPage = ({ entityId }: { entityId: string }) => {
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
  const contexts = useQuery({
    queryKey: entityQueryKeys.contexts(),
    queryFn: listContexts,
  });
  const contextId =
    selectedContext ||
    (contexts.data?.find((context) => context.code === 'default')?.id ?? null);
  return (
    <EntityPage title="Edit entity">
      <Box sx={{ mt: 1 }}>
        <Link params={{ entityId }} to="/entities/$entityId">
          View preview
        </Link>
      </Box>
      {entityForm.isPending && (
        <Typography sx={{ mt: 4 }}>Loading entity...</Typography>
      )}
      {(entityForm.error || update.error) && (
        <Alert severity="error" sx={{ mt: 4 }}>
          {(entityForm.error ?? update.error)?.message}
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
            value={selectedContext}
          >
            {contexts.data?.map((context) => (
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
    </EntityPage>
  );
};

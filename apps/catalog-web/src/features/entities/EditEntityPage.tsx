import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Button, MenuItem, TextField, Typography } from '@mui/material';
import { useState } from 'react';
import {
  createContext,
  getEntityForm,
  listContexts,
  updateEntity,
} from './api';
import { EntityForm } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { valuesForForm } from './entity-form';
import { entityQueryKeys } from './query-keys';

export const EditEntityPage = ({ entityId }: { entityId: string }) => {
  const navigate = useNavigate({ from: '/entities/$entityId/edit' });
  const queryClient = useQueryClient();
  const [contextId, setContextId] = useState<string | null>(null);
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
  const create = useMutation({
    mutationFn: ({
      code,
      data,
    }: {
      code: string;
      data: Record<string, unknown>;
    }) => createContext(code, data),
    onSuccess: (context) => {
      void queryClient.invalidateQueries({
        queryKey: entityQueryKeys.contexts(),
      });
      setContextId(context.id);
    },
  });
  const addContext = () => {
    const code = window.prompt('Context code');
    if (!code?.trim()) return;
    const rawData = window.prompt('Context metadata as a JSON object', '{}');
    if (rawData === null) return;
    try {
      const data: unknown = JSON.parse(rawData);
      if (typeof data !== 'object' || data === null || Array.isArray(data)) {
        throw new Error('Context metadata must be a JSON object');
      }
      create.mutate({
        code: code.trim(),
        data: data as Record<string, unknown>,
      });
    } catch (error) {
      window.alert(
        error instanceof Error ? error.message : 'Invalid context metadata',
      );
    }
  };
  return (
    <EntityPage title="Edit entity">
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
            onChange={(event) => setContextId(event.target.value || null)}
            sx={{ mt: 4 }}
            value={contextId ?? ''}
          >
            <MenuItem value="">Default</MenuItem>
            {contexts.data?.map((context) => (
              <MenuItem key={context.id} value={context.id}>
                {context.code}
              </MenuItem>
            ))}
          </TextField>
          <Button onClick={addContext} sx={{ mt: 1 }} variant="outlined">
            Add context
          </Button>
          {create.error && (
            <Alert severity="error">{create.error.message}</Alert>
          )}
          <EntityForm
            key={`${entityForm.data.entity.id}:${contextId}`}
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

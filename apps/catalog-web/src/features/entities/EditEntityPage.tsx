import { useMutation, useQuery } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { Alert, Typography } from '@mui/material';
import { getEntityForm, updateEntity } from './api';
import { EntityForm } from './components/EntityForm';
import { EntityPage } from './components/EntityPage';
import { valuesForForm } from './entity-form';
import { entityQueryKeys } from './query-keys';

export const EditEntityPage = ({ entityId }: { entityId: string }) => {
  const navigate = useNavigate({ from: '/entities/$entityId/edit' });
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
        <EntityForm
          key={entityForm.data.entity.id}
          blueprint={entityForm.data.blueprint}
          initialValues={valuesForForm(
            entityForm.data.blueprint.attributes,
            entityForm.data.values,
          )}
          isLoadingBlueprint={update.isPending}
          onSubmit={(input) => update.mutate(input)}
          submitLabel="Save changes"
        />
      )}
    </EntityPage>
  );
};

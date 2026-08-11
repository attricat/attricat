import { useMutation, useQuery } from '@tanstack/react-query';
import { createFileRoute, useNavigate } from '@tanstack/react-router';
import { Alert, Typography } from '@mui/material';
import { getEntityForm, updateEntity } from '../../../api';
import { EntityForm } from '../../../EntityForm';
import { EntityPage } from '../../../EntityPage';
import { valuesForForm } from '../../../entity-form';

export const Route = createFileRoute('/entities/$entityId/edit')({
  component: () => <EditEntity />,
});

const EditEntity = () => {
  const { entityId } = Route.useParams();
  const navigate = useNavigate({ from: Route.fullPath });
  const entityForm = useQuery({
    queryKey: ['entity-form', entityId],
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

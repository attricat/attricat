import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { createReusableAttributeGroup, type ReusableAttribute } from './api';
import {
  DEFAULT_GROUP_POSITION,
  GROUP_POSITION_STEP,
  MIN_GROUP_POSITION,
  STATUS_PUBLISHED,
} from './constants';
import { reusableAttributeQueryKeys } from './queryKeys';

export const ReusableAttributeGroupDialog = ({
  attributes,
  onClose,
}: {
  attributes: ReusableAttribute[];
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [validationError, setValidationError] = useState<string>();
  const save = useMutation({
    mutationFn: createReusableAttributeGroup,
    onSuccess: async () => {
      await queryClient.invalidateQueries({
        queryKey: reusableAttributeQueryKeys.groups(),
      });
      onClose();
    },
  });
  const form = useForm({
    defaultValues: {
      code: '',
      name: '',
      position: DEFAULT_GROUP_POSITION,
      revisionIds: [] as string[],
    },
    onSubmit: ({ value }) => {
      setValidationError(undefined);
      if (
        !value.code.trim() ||
        !value.name.trim() ||
        !value.revisionIds.length
      ) {
        setValidationError(t('reusableAttributes.groupDialog.requiredFields'));
        return;
      }
      const position = Number(value.position);
      if (!Number.isInteger(position) || position < MIN_GROUP_POSITION) {
        setValidationError(t('reusableAttributes.groupDialog.invalidPosition'));
        return;
      }
      save.mutate({
        code: value.code.trim(),
        name: value.name.trim(),
        position,
        reusable_attribute_revision_ids: value.revisionIds,
      });
    },
  });
  const published = attributes.filter(
    (attribute) => attribute.status === STATUS_PUBLISHED,
  );
  return (
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={() => {
        if (!save.isPending) onClose();
      }}
      open
    >
      <DialogTitle>{t('reusableAttributes.groupDialog.title')}</DialogTitle>
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            <form.Field name="code">
              {(field) => (
                <TextField
                  label={t('reusableAttributes.groupDialog.code')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="name">
              {(field) => (
                <TextField
                  label={t('reusableAttributes.groupDialog.name')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="position">
              {(field) => (
                <TextField
                  label={t('reusableAttributes.groupDialog.position')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  slotProps={{
                    htmlInput: {
                      min: MIN_GROUP_POSITION,
                      step: GROUP_POSITION_STEP,
                    },
                  }}
                  type="number"
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="revisionIds">
              {(field) => (
                <TextField
                  helperText={t('reusableAttributes.groupDialog.revisionsHelp')}
                  label={t('reusableAttributes.groupDialog.revisions')}
                  onChange={(event) =>
                    field.handleChange(
                      event.target.value as unknown as string[],
                    )
                  }
                  select
                  slotProps={{ select: { multiple: true } }}
                  value={field.state.value}
                >
                  {published.map((attribute) => (
                    <MenuItem key={attribute.id} value={attribute.id}>
                      {attribute.namespace}:{attribute.code} · v
                      {attribute.version}
                    </MenuItem>
                  ))}
                </TextField>
              )}
            </form.Field>
            {(validationError || save.error) && (
              <Alert severity="error">
                {validationError ?? save.error?.message}
              </Alert>
            )}
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button disabled={save.isPending} onClick={onClose}>
            {t('reusableAttributes.groupDialog.cancel')}
          </Button>
          <Button
            disabled={save.isPending || !published.length}
            type="submit"
            variant="contained"
          >
            {t('reusableAttributes.groupDialog.create')}
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
};

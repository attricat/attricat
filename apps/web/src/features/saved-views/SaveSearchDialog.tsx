import { useForm } from '@tanstack/react-form';
import { MenuItem, Stack, TextField } from '@mui/material';
import { FormDialog } from '../../components/FormDialog';
import { LockIcon, UsersIcon } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { IconLabel } from '../../components/IconLabel';
import {
  SAVED_VIEW_DESCRIPTION_MAX_LENGTH,
  SAVED_VIEW_NAME_MAX_LENGTH,
  SAVED_VIEW_VISIBILITY_PRIVATE,
  SAVED_VIEW_VISIBILITY_WORKSPACE,
  type SavedViewVisibility,
} from './constants';

export type SaveSearchValues = {
  name: string;
  description: string;
  visibility: SavedViewVisibility;
};

export const SaveSearchDialog = ({
  error,
  isPending,
  onClose,
  onSave,
  title,
}: {
  error: Error | null;
  isPending: boolean;
  onClose: () => void;
  onSave: (values: SaveSearchValues) => void;
  title?: string;
}) => {
  const { t } = useTranslation();
  const form = useForm({
    defaultValues: {
      name: '',
      description: '',
      visibility: SAVED_VIEW_VISIBILITY_PRIVATE as SavedViewVisibility,
    },
    onSubmit: ({ value }) => onSave({ ...value, name: value.name.trim() }),
  });

  return (
    <FormDialog
      error={error}
      isPending={isPending}
      onClose={onClose}
      onSubmit={() => void form.handleSubmit()}
      submitLabel={t('explorer.saveSearch')}
      title={title ?? t('explorer.saveSearch')}
    >
      <Stack spacing={2} sx={{ pt: 1 }}>
        <form.Field
          name="name"
          validators={{
            // Spaces alone are no name.
            onSubmit: ({ value }) =>
              value.trim() ? undefined : t('validation.required'),
          }}
        >
          {(field) => (
            <TextField
              required
              error={field.state.meta.errors.length > 0}
              helperText={field.state.meta.errors[0]}
              label={t('explorer.searchName')}
              slotProps={{
                htmlInput: { maxLength: SAVED_VIEW_NAME_MAX_LENGTH },
              }}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
            />
          )}
        </form.Field>
        <form.Field name="description">
          {(field) => (
            <TextField
              label={t('explorer.searchDescription')}
              slotProps={{
                htmlInput: {
                  maxLength: SAVED_VIEW_DESCRIPTION_MAX_LENGTH,
                },
              }}
              value={field.state.value}
              onChange={(event) => field.handleChange(event.target.value)}
            />
          )}
        </form.Field>
        <form.Field name="visibility">
          {(field) => (
            <TextField
              select
              label={t('explorer.searchVisibility')}
              value={field.state.value}
              onChange={(event) =>
                field.handleChange(event.target.value as SavedViewVisibility)
              }
            >
              <MenuItem value={SAVED_VIEW_VISIBILITY_PRIVATE}>
                <IconLabel icon={LockIcon}>
                  {t('explorer.privateSearch')}
                </IconLabel>
              </MenuItem>
              <MenuItem value={SAVED_VIEW_VISIBILITY_WORKSPACE}>
                <IconLabel icon={UsersIcon}>
                  {t('explorer.workspaceSearch')}
                </IconLabel>
              </MenuItem>
            </TextField>
          )}
        </form.Field>
      </Stack>
    </FormDialog>
  );
};

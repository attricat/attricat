import { useForm } from '@tanstack/react-form';
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
import { useTranslation } from 'react-i18next';
import {
  SAVE_SEARCH_FORM_ID,
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
  open,
  title,
}: {
  error: string;
  isPending: boolean;
  onClose: () => void;
  onSave: (values: SaveSearchValues) => void;
  open: boolean;
  title?: string;
}) => {
  const { t } = useTranslation();
  const form = useForm({
    defaultValues: {
      name: '',
      description: '',
      visibility: SAVED_VIEW_VISIBILITY_PRIVATE as SavedViewVisibility,
    },
    onSubmit: ({ value }) => {
      if (value.name.trim()) onSave({ ...value, name: value.name.trim() });
    },
  });

  return (
    <Dialog open={open} onClose={onClose} fullWidth>
      <DialogTitle>{title ?? t('explorer.saveSearch')}</DialogTitle>
      <DialogContent>
        <Box
          component="form"
          id={SAVE_SEARCH_FORM_ID}
          onSubmit={(event) => {
            event.preventDefault();
            void form.handleSubmit();
          }}
        >
          <Stack spacing={2} sx={{ pt: 1 }}>
            <form.Field name="name">
              {(field) => (
                <TextField
                  required
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
                    field.handleChange(
                      event.target.value as SavedViewVisibility,
                    )
                  }
                >
                  <MenuItem value={SAVED_VIEW_VISIBILITY_PRIVATE}>
                    {t('explorer.privateSearch')}
                  </MenuItem>
                  <MenuItem value={SAVED_VIEW_VISIBILITY_WORKSPACE}>
                    {t('explorer.workspaceSearch')}
                  </MenuItem>
                </TextField>
              )}
            </form.Field>
            {error && <Alert severity="error">{error}</Alert>}
          </Stack>
        </Box>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('common.cancel')}</Button>
        <Button type="submit" form={SAVE_SEARCH_FORM_ID} disabled={isPending}>
          {t('explorer.saveSearch')}
        </Button>
      </DialogActions>
    </Dialog>
  );
};

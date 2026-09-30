import { useRef } from 'react';
import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  TextField,
} from '@mui/material';
import { updateDisplayName } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import {
  displayNameCharacters,
  displayNameMaxLength,
  displayNameMinLength,
} from './constants';

export const ChangeDisplayNameDialog = ({
  initialName,
  onClose,
}: {
  initialName: string;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const inputRef = useRef<HTMLInputElement>(null);
  const save = useMutation({
    mutationFn: (name: string) => updateDisplayName(name),
    onSuccess: (session) => {
      client.setQueryData(authQueryKeys.session(), session);
      onClose();
    },
  });
  const validateName = (value: string) => {
    // Code points, matching the API's character count.
    const length = [...value].length;
    if (length < displayNameMinLength)
      return t('profile.displayNameTooShort', { min: displayNameMinLength });
    if (length > displayNameMaxLength)
      return t('profile.displayNameTooLong', { max: displayNameMaxLength });
    if (!displayNameCharacters.test(value))
      return t('profile.displayNameInvalidCharacters');
    if (value.startsWith(' ') || value.endsWith(' '))
      return t('profile.displayNameEdgeSpace');
    return undefined;
  };
  const form = useForm({
    defaultValues: { displayName: initialName },
    onSubmit: ({ value }) => save.mutate(value.displayName),
  });
  return (
    <Dialog
      fullWidth
      maxWidth="sm"
      onClose={() => {
        if (!save.isPending) onClose();
      }}
      open
      slotProps={{ transition: { onEntered: () => inputRef.current?.focus() } }}
    >
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <DialogTitle>{t('profile.changeDisplayName')}</DialogTitle>
        <DialogContent>
          <form.Field
            name="displayName"
            validators={{
              onChange: ({ value }) => validateName(value),
              onSubmit: ({ value }) => validateName(value),
            }}
          >
            {(field) => (
              <TextField
                autoComplete="name"
                error={field.state.meta.errors.length > 0}
                fullWidth
                helperText={
                  field.state.meta.errors[0] ??
                  t('profile.displayNameHint', {
                    min: displayNameMinLength,
                    max: displayNameMaxLength,
                  })
                }
                inputRef={inputRef}
                label={t('profile.displayName')}
                margin="dense"
                onBlur={field.handleBlur}
                onChange={(event) => field.handleChange(event.target.value)}
                value={field.state.value}
              />
            )}
          </form.Field>
          {save.error && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {save.error.message}
            </Alert>
          )}
        </DialogContent>
        <DialogActions>
          <Button disabled={save.isPending} onClick={onClose}>
            {t('common.cancel')}
          </Button>
          <Button disabled={save.isPending} type="submit" variant="contained">
            {t('profile.saveDisplayName')}
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
};

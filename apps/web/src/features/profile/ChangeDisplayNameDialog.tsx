import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { Button, TextField } from '@mui/material';
import { clearDisplayName, updateDisplayName } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import {
  displayNameCharacters,
  displayNameMaxLength,
  displayNameMinLength,
} from './constants';
import { FormDialog } from '../../components/FormDialog';

export const ChangeDisplayNameDialog = ({
  initialName,
  onClose,
}: {
  initialName: string;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  // An empty name clears it, so the account is shown by its email.
  const save = useMutation({
    mutationFn: (name: string | null) =>
      name === null ? clearDisplayName() : updateDisplayName(name),
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
    <FormDialog
      error={save.error}
      isPending={save.isPending}
      onClose={onClose}
      onSubmit={() => void form.handleSubmit()}
      submitLabel={t('profile.saveDisplayName')}
      title={t('profile.changeDisplayName')}
    >
      <form.Field
        name="displayName"
        validators={{
          onChange: ({ value }) => validateName(value),
          onSubmit: ({ value }) => validateName(value),
        }}
      >
        {(field) => (
          <TextField
            required
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
            label={t('profile.displayName')}
            margin="dense"
            onBlur={field.handleBlur}
            onChange={(event) => field.handleChange(event.target.value)}
            value={field.state.value}
          />
        )}
      </form.Field>
      {initialName && (
        <Button
          color="error"
          disabled={save.isPending}
          onClick={() => save.mutate(null)}
          size="small"
        >
          {t('profile.removeDisplayName')}
        </Button>
      )}
    </FormDialog>
  );
};

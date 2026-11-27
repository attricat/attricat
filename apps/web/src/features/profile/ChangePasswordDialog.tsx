import { useForm } from '@tanstack/react-form';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { TextField } from '@mui/material';
import { changePassword } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { FormDialog } from '../../components/FormDialog';
import { toast } from '../../components/toast';

type PasswordFields = {
  currentPassword: string;
  newPassword: string;
  confirmPassword: string;
};

const passwordFields = [
  {
    name: 'currentPassword',
    labelKey: 'profile.currentPassword',
    autoComplete: 'current-password',
  },
  {
    name: 'newPassword',
    labelKey: 'auth.newPassword',
    autoComplete: 'new-password',
  },
  {
    name: 'confirmPassword',
    labelKey: 'auth.confirmNewPassword',
    autoComplete: 'new-password',
  },
] as const satisfies readonly {
  name: keyof PasswordFields;
  labelKey: string;
  autoComplete: string;
}[];

export const ChangePasswordDialog = ({ onClose }: { onClose: () => void }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const save = useMutation({
    meta: { toast: false },
    mutationFn: ({ currentPassword, newPassword }: PasswordFields) =>
      changePassword(currentPassword, newPassword),
    onSuccess: (session) => {
      // The response replaced this browser's session cookie.
      client.setQueryData(authQueryKeys.session(), session);
      toast.success(t('profile.passwordChanged'));
      onClose();
    },
  });
  const form = useForm({
    defaultValues: {
      currentPassword: '',
      newPassword: '',
      confirmPassword: '',
    } satisfies PasswordFields,
    onSubmit: ({ value }) => save.mutate(value),
  });
  return (
    <FormDialog
      error={save.error}
      isPending={save.isPending}
      onClose={onClose}
      onSubmit={() => void form.handleSubmit()}
      submitLabel={t('profile.changePassword')}
      title={t('profile.changePassword')}
    >
      {passwordFields.map(({ name, labelKey, autoComplete }) => (
        <form.Field
          key={name}
          name={name}
          validators={
            name === 'confirmPassword'
              ? {
                  onSubmit: ({ value, fieldApi }) =>
                    value === fieldApi.form.getFieldValue('newPassword')
                      ? undefined
                      : t('workspace.passwordMismatch'),
                }
              : undefined
          }
        >
          {(field) => (
            <TextField
              autoComplete={autoComplete}
              disabled={save.isPending}
              error={field.state.meta.errors.length > 0}
              fullWidth
              helperText={field.state.meta.errors[0]}
              label={t(labelKey)}
              margin="dense"
              onBlur={field.handleBlur}
              onChange={(event) => field.handleChange(event.target.value)}
              required
              type="password"
              value={field.state.value}
            />
          )}
        </form.Field>
      ))}
    </FormDialog>
  );
};

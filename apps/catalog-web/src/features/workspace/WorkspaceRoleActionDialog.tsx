import { useForm } from '@tanstack/react-form';
import { useMutation } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  TextField,
} from '@mui/material';
import { useRef } from 'react';
import { useTranslation } from 'react-i18next';
import {
  duplicateRole,
  retireRole,
  updateRole,
  type WorkspaceRole,
} from './api';

export type RoleDialogAction =
  | { role: WorkspaceRole; type: 'rename' }
  | { role: WorkspaceRole; type: 'duplicate' }
  | { role: WorkspaceRole; type: 'retire' };

const roleCodePattern = /^[a-z][a-z0-9_-]*$/;
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export const WorkspaceRoleActionDialog = ({
  action,
  onClose,
  onSuccess,
}: {
  action: RoleDialogAction;
  onClose: () => void;
  onSuccess: () => void;
}) => {
  const { t } = useTranslation();
  const initialFocusRef = useRef<HTMLInputElement>(null);
  const isRetiring = action.type === 'retire';
  const mutation = useMutation({
    mutationFn: async (value: { code: string; replacementRoleId: string }) => {
      if (action.type === 'rename') {
        await updateRole(action.role.id, {
          code: value.code,
          permissions: action.role.permissions,
        });
        return;
      }
      if (action.type === 'duplicate') {
        await duplicateRole(action.role.id, value.code);
        return;
      }
      await retireRole(action.role.id, value.replacementRoleId || undefined);
    },
    onSuccess,
  });
  const form = useForm({
    defaultValues: {
      code: action.type === 'rename' ? action.role.code : '',
      replacementRoleId: '',
    },
    onSubmit: ({ value }) => mutation.mutate(value),
  });
  const codeError = (value: string) =>
    roleCodePattern.test(value) ? undefined : t('workspace.roleCodeInvalid');
  const replacementRoleError = (value: string) =>
    !value || uuidPattern.test(value)
      ? undefined
      : t('workspace.replacementRoleInvalid');
  const title =
    action.type === 'rename'
      ? t('workspace.renameRole')
      : action.type === 'duplicate'
        ? t('workspace.duplicateRole')
        : t('workspace.retireRole');

  return (
    <Dialog
      aria-describedby="role-action-dialog-description"
      fullWidth
      maxWidth="sm"
      onClose={(_, reason) => {
        if (!mutation.isPending && reason !== 'backdropClick') onClose();
      }}
      open
      slotProps={{
        transition: {
          onEntered: () => initialFocusRef.current?.focus(),
        },
      }}
    >
      <Box
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
      >
        <DialogTitle>{title}</DialogTitle>
        <DialogContent>
          <DialogContentText id="role-action-dialog-description">
            {isRetiring
              ? t('workspace.retireRoleDescription', { role: action.role.code })
              : t('workspace.roleActionDescription', {
                  role: action.role.code,
                })}
          </DialogContentText>
          {isRetiring ? (
            <form.Field
              name="replacementRoleId"
              validators={{
                onChange: ({ value }) => replacementRoleError(value),
                onSubmit: ({ value }) => replacementRoleError(value),
              }}
            >
              {(field) => (
                <TextField
                  error={field.state.meta.errors.length > 0}
                  fullWidth
                  helperText={
                    field.state.meta.errors[0] ?? t('workspace.replacementRole')
                  }
                  inputRef={initialFocusRef}
                  label={t('workspace.replacementRoleLabel')}
                  margin="dense"
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  value={field.state.value}
                />
              )}
            </form.Field>
          ) : (
            <form.Field
              name="code"
              validators={{
                onChange: ({ value }) => codeError(value),
                onSubmit: ({ value }) => codeError(value),
              }}
            >
              {(field) => (
                <TextField
                  error={field.state.meta.errors.length > 0}
                  fullWidth
                  helperText={field.state.meta.errors[0]}
                  inputRef={initialFocusRef}
                  required
                  label={t(
                    action.type === 'rename'
                      ? 'workspace.roleCode'
                      : 'workspace.newRoleCode',
                  )}
                  margin="dense"
                  onBlur={field.handleBlur}
                  onChange={(event) => field.handleChange(event.target.value)}
                  value={field.state.value}
                />
              )}
            </form.Field>
          )}
          {mutation.isError && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {mutation.error.message}
            </Alert>
          )}
        </DialogContent>
        <DialogActions>
          <Button disabled={mutation.isPending} onClick={onClose}>
            {t('workspace.cancel')}
          </Button>
          <Button
            color={isRetiring ? 'error' : 'primary'}
            disabled={mutation.isPending}
            type="submit"
            variant="contained"
          >
            {mutation.isPending ? t('workspace.saving') : title}
          </Button>
        </DialogActions>
      </Box>
    </Dialog>
  );
};

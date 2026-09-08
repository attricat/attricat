import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  FormControlLabel,
  List,
  ListItem,
  ListItemText,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createRole,
  duplicateRole,
  listPermissions,
  listRoles,
  retireRole,
  updateRole,
  type WorkspaceRole,
} from './api';
import { workspaceQueryKeys } from './query-keys';

type RoleDialogAction =
  | { role: WorkspaceRole; type: 'rename' }
  | { role: WorkspaceRole; type: 'duplicate' }
  | { role: WorkspaceRole; type: 'retire' };

const roleCodePattern = /^[a-z][a-z0-9_-]*$/;
const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

export const WorkspaceRolesSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [error, setError] = useState<string>();
  const [dialogAction, setDialogAction] = useState<RoleDialogAction>();
  const roles = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.roles(),
    queryFn: listRoles,
  });
  const permissions = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.permissions(),
    queryFn: listPermissions,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: workspaceQueryKeys.roles() });
  const form = useForm({
    defaultValues: { code: '', permissions: [] as string[] },
    onSubmit: async ({ value }) => {
      try {
        await createRole(value);
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : t('workspace.createRoleFailed'),
        );
      }
    },
  });
  if (!canManage) {
    return <Alert severity="error">{t('workspace.notAuthorizedRoles')}</Alert>;
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      {error && <Alert severity="error">{error}</Alert>}
      {roles.isError && <Alert severity="error">{roles.error.message}</Alert>}
      {permissions.isError && (
        <Alert severity="error">{permissions.error.message}</Alert>
      )}
      <Paper>
        <List>
          {roles.data?.map((role) => (
            <ListItem
              divider
              key={role.id}
              secondaryAction={
                !role.is_system && (
                  <Stack direction="row">
                    <Button
                      onClick={() => setDialogAction({ role, type: 'rename' })}
                    >
                      {t('workspace.renameRole')}
                    </Button>
                    <Button
                      onClick={() =>
                        setDialogAction({ role, type: 'duplicate' })
                      }
                    >
                      {t('workspace.duplicateRole')}
                    </Button>
                    <Button
                      color="error"
                      onClick={() => setDialogAction({ role, type: 'retire' })}
                    >
                      {t('workspace.retireRole')}
                    </Button>
                  </Stack>
                )
              }
            >
              <ListItemText
                primary={`${role.code}${role.is_system ? t('workspace.fixed') : ''}`}
                secondary={role.permissions.join(', ')}
              />
            </ListItem>
          ))}
        </List>
      </Paper>
      {dialogAction && (
        <RoleActionDialog
          action={dialogAction}
          onClose={() => setDialogAction(undefined)}
          onSuccess={() => {
            void refresh();
            setDialogAction(undefined);
          }}
        />
      )}
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          form.handleSubmit();
        }}
        sx={{ p: 2 }}
      >
        <Stack>
          <Typography variant="h6">
            {t('workspace.createCustomRole')}
          </Typography>
          <form.Field name="code">
            {(field) => (
              <TextField
                label={t('workspace.roleCode')}
                onChange={(event) => field.handleChange(event.target.value)}
                value={field.state.value}
              />
            )}
          </form.Field>
          <form.Field name="permissions">
            {(field) => (
              <>
                {permissions.data?.map((permission) => (
                  <FormControlLabel
                    control={
                      <Checkbox
                        checked={field.state.value.includes(permission.code)}
                        onChange={(event) =>
                          field.handleChange(
                            event.target.checked
                              ? [...field.state.value, permission.code]
                              : field.state.value.filter(
                                  (code) => code !== permission.code,
                                ),
                          )
                        }
                      />
                    }
                    key={permission.code}
                    label={`${permission.code} — ${permission.description}`}
                  />
                ))}
              </>
            )}
          </form.Field>
          <Button type="submit" variant="contained">
            Create role
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};

const RoleActionDialog = ({
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

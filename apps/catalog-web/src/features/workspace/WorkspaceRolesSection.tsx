import { useForm } from '@tanstack/react-form';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Checkbox,
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
import { authQueryKeys } from '../auth/queryKeys';
import { useTranslation } from 'react-i18next';
import { ArchiveIcon, CopyIcon, PencilIcon } from 'lucide-react';
import { createRole, listPermissions, listRoles } from './api';
import { workspaceQueryKeys } from './queryKeys';
import {
  WorkspaceRoleActionDialog,
  type RoleDialogAction,
} from './WorkspaceRoleActionDialog';

export const WorkspaceRolesSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [error, setError] = useState<string>();
  const submitting = useRef(false);
  const [isSubmitting, setIsSubmitting] = useState(false);
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
  const refresh = () => {
    void client.invalidateQueries({ queryKey: authQueryKeys.session() });
    return client.invalidateQueries({ queryKey: workspaceQueryKeys.roles() });
  };
  const form = useForm({
    defaultValues: { code: '', permissions: [] as string[] },
    onSubmit: async ({ value }) => {
      if (submitting.current) return;
      submitting.current = true;
      setIsSubmitting(true);
      setError(undefined);
      try {
        await createRole(value);
        void refresh();
        form.reset();
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : t('workspace.createRoleFailed'),
        );
      } finally {
        submitting.current = false;
        setIsSubmitting(false);
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
                      startIcon={<PencilIcon />}
                    >
                      {t('workspace.renameRole')}
                    </Button>
                    <Button
                      onClick={() =>
                        setDialogAction({ role, type: 'duplicate' })
                      }
                      startIcon={<CopyIcon />}
                    >
                      {t('workspace.duplicateRole')}
                    </Button>
                    <Button
                      color="error"
                      onClick={() => setDialogAction({ role, type: 'retire' })}
                      startIcon={<ArchiveIcon />}
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
        <WorkspaceRoleActionDialog
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
          <Button disabled={isSubmitting} type="submit" variant="contained">
            {t('workspace.createRole')}
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};

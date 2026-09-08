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
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  createRole,
  duplicateRole,
  listPermissions,
  listRoles,
  retireRole,
  updateRole,
} from './api';
import { workspaceQueryKeys } from './query-keys';

export const WorkspaceRolesSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [error, setError] = useState<string>();
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
                      onClick={() => {
                        const code = window.prompt(
                          t('workspace.roleCode'),
                          role.code,
                        );
                        if (code)
                          updateRole(role.id, {
                            code,
                            permissions: role.permissions,
                          })
                            .then(refresh)
                            .catch((e) => setError(e.message));
                      }}
                    >
                      Rename
                    </Button>
                    <Button
                      onClick={() => {
                        const code = window.prompt(t('workspace.newRoleCode'));
                        if (code)
                          duplicateRole(role.id, code)
                            .then(refresh)
                            .catch((e) => setError(e.message));
                      }}
                    >
                      Duplicate
                    </Button>
                    <Button
                      color="error"
                      onClick={() => {
                        const replacement = window.prompt(
                          t('workspace.replacementRole'),
                        );
                        retireRole(role.id, replacement || undefined)
                          .then(refresh)
                          .catch((e) => setError(e.message));
                      }}
                    >
                      Retire
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

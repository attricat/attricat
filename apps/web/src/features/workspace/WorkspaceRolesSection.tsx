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
import { invalidateAuthorization } from '../auth/invalidateAuthorization';
import { useTranslation } from 'react-i18next';
import { ArchiveIcon, CopyIcon, PencilIcon } from 'lucide-react';
import { createRole, listPermissions } from './api';
import { workspaceQueryKeys } from './queryKeys';
import {
  WorkspaceRoleActionDialog,
  type RoleDialogAction,
} from './WorkspaceRoleActionDialog';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { useExclusiveMutation } from '../../components/useExclusiveMutation';
import { roleListOptions } from './queryOptions';
import { roleCodePattern } from './constants';

export const WorkspaceRolesSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [dialogAction, setDialogAction] = useState<RoleDialogAction>();
  const roles = useQuery({
    enabled: canManage,
    ...roleListOptions(),
  });
  const permissions = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.permissions(),
    queryFn: listPermissions,
  });
  const refresh = () => {
    void invalidateAuthorization(client);
    return client.invalidateQueries({ queryKey: workspaceQueryKeys.roles() });
  };
  const create = useExclusiveMutation({
    mutationFn: createRole,
    onSuccess: () => {
      void refresh();
      form.reset();
    },
  });
  const form = useForm({
    defaultValues: { code: '', permissions: [] as string[] },
    onSubmit: ({ value }) => create.submit(value),
  });
  if (!canManage) {
    return <Alert severity="error">{t('workspace.notAuthorizedRoles')}</Alert>;
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <ApiErrorAlert error={create.error} />
      <ApiErrorAlert error={roles.error} />
      <ApiErrorAlert error={permissions.error} />
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
          <Typography component="h2" variant="h6">
            {t('workspace.createCustomRole')}
          </Typography>
          <form.Field
            name="code"
            validators={{
              onSubmit: ({ value }) =>
                roleCodePattern.test(value)
                  ? undefined
                  : t('workspace.roleCodeInvalid'),
            }}
          >
            {(field) => (
              <TextField
                error={field.state.meta.errors.length > 0}
                helperText={field.state.meta.errors[0]}
                required
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
          <Button disabled={create.isPending} type="submit" variant="contained">
            {t('workspace.createRole')}
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};

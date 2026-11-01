import { useForm } from '@tanstack/react-form';
import { useMutation } from '@tanstack/react-query';
import {
  Checkbox,
  FormControlLabel,
  FormGroup,
  MenuItem,
  TextField,
} from '@mui/material';
import { useTranslation } from 'react-i18next';
import type { z } from 'zod';
import { roleCodePattern } from './constants';
import {
  duplicateRole,
  retireRole,
  updateRole,
  type WorkspaceRole,
} from './api';
import { FormDialog } from '../../components/FormDialog';
import type { permissionSchema } from './schemas';
import { permissionDescription } from './permissionDescription';

export type RoleDialogAction =
  | { role: WorkspaceRole; type: 'rename' }
  | { role: WorkspaceRole; type: 'duplicate' }
  | { role: WorkspaceRole; type: 'retire' };

export const WorkspaceRoleActionDialog = ({
  action,
  onClose,
  onSuccess,
  permissions = [],
  roles = [],
}: {
  action: RoleDialogAction;
  onClose: () => void;
  onSuccess: () => void;
  /** Every permission a custom role can hold, for editing one. */
  permissions?: readonly z.infer<typeof permissionSchema>[];
  /** The workspace's roles; others are offered as a retired role's replacement. */
  roles?: readonly WorkspaceRole[];
}) => {
  const { t } = useTranslation();
  const isRetiring = action.type === 'retire';
  const mutation = useMutation({
    mutationFn: async (value: {
      code: string;
      permissions: string[];
      replacementRoleId: string;
    }) => {
      if (action.type === 'rename') {
        await updateRole(action.role.id, {
          code: value.code,
          permissions: value.permissions,
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
      permissions: [...action.role.permissions],
      replacementRoleId: '',
    },
    onSubmit: ({ value }) => mutation.mutate(value),
  });
  const codeError = (value: string) =>
    roleCodePattern.test(value) ? undefined : t('workspace.roleCodeInvalid');
  const title =
    action.type === 'rename'
      ? t('workspace.renameRole')
      : action.type === 'duplicate'
        ? t('workspace.duplicateRole')
        : t('workspace.retireRole');

  return (
    <FormDialog
      description={
        isRetiring
          ? t('workspace.retireRoleDescription', { role: action.role.code })
          : t('workspace.roleActionDescription', { role: action.role.code })
      }
      destructive={isRetiring}
      error={mutation.error}
      isPending={mutation.isPending}
      onClose={onClose}
      onSubmit={() => void form.handleSubmit()}
      slotProps={{
        transition: {
          // The first field: the code, or the replacement role select.
          onEntered: (node) =>
            node
              .querySelector<HTMLElement>(
                'input[type="text"], [role="combobox"]',
              )
              ?.focus(),
        },
      }}
      submitLabel={mutation.isPending ? t('workspace.saving') : title}
      title={title}
    >
      {isRetiring ? (
        <form.Field name="replacementRoleId">
          {(field) => (
            <TextField
              fullWidth
              helperText={t('workspace.replacementRole')}
              label={t('workspace.replacementRoleLabel')}
              margin="dense"
              onChange={(event) => field.handleChange(event.target.value)}
              select
              slotProps={{
                select: { displayEmpty: true },
                inputLabel: { shrink: true },
              }}
              value={field.state.value}
            >
              <MenuItem value="">{t('workspace.noReplacementRole')}</MenuItem>
              {roles
                .filter((role) => role.id !== action.role.id)
                .map((role) => (
                  <MenuItem key={role.id} value={role.id}>
                    {role.code}
                  </MenuItem>
                ))}
            </TextField>
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
      {action.type === 'rename' && (
        <form.Field name="permissions">
          {(field) => (
            <FormGroup sx={{ mt: 1 }}>
              {permissions.map((permission) => (
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
                  label={`${permission.code} — ${permissionDescription(t, permission)}`}
                />
              ))}
            </FormGroup>
          )}
        </form.Field>
      )}
    </FormDialog>
  );
};

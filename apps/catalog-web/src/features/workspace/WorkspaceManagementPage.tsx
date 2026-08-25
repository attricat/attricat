import { useForm } from '@tanstack/react-form';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Dialog,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  List,
  ListItem,
  ListItemText,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { currentSession } from '../auth/api';
import {
  acceptInvitation,
  createInvitation,
  createRole,
  createToken,
  duplicateRole,
  ensureActiveScopeTarget,
  grantMemberRole,
  listAssignableRoles,
  listGrantTargets,
  listInvitations,
  listMembers,
  listPermissions,
  listRoles,
  listTokenPermissions,
  listTokens,
  retireRole,
  revokeInvitation,
  revokeMemberRole,
  revokeToken,
  setMemberState,
  transferOwnership,
  updateRole,
  type ScopeType,
} from './api';
import { workspaceQueryKeys } from './query-keys';

type Section = 'members' | 'roles' | 'invitations' | 'tokens';
const sections: { label: string; section: Section; to: string }[] = [
  { label: 'Members', section: 'members', to: '/workspace/members' },
  { label: 'Roles', section: 'roles', to: '/workspace/roles' },
  {
    label: 'Invitations',
    section: 'invitations',
    to: '/workspace/invitations',
  },
  { label: 'Personal tokens', section: 'tokens', to: '/workspace/tokens' },
];

const SecretDialog = ({
  secret,
  onClose,
}: {
  secret?: string;
  onClose: () => void;
}) => (
  <Dialog onClose={onClose} open={Boolean(secret)}>
    <DialogTitle>Copy this secret now</DialogTitle>
    <DialogContent>
      <Stack spacing={2} sx={{ minWidth: 360 }}>
        <Alert severity="warning">
          This secret is shown only once. Store it securely before closing this
          dialog.
        </Alert>
        <TextField
          slotProps={{ input: { readOnly: true } }}
          value={secret ?? ''}
        />
        <Button
          onClick={() => navigator.clipboard?.writeText(secret ?? '')}
          variant="contained"
        >
          Copy secret
        </Button>
      </Stack>
    </DialogContent>
  </Dialog>
);

const ScopeFields = ({
  form,
  workspaceId,
}: {
  form: any;
  workspaceId?: string;
}) => (
  <>
    <form.Field name="scope_type">
      {(field: any) => (
        <TextField
          fullWidth
          label="Scope"
          onChange={(event) => {
            const scope = event.target.value as ScopeType;
            field.handleChange(scope);
            form.setFieldValue(
              'scope_target_id',
              scope === 'workspace' ? (workspaceId ?? '') : '',
            );
          }}
          select
          value={field.state.value}
        >
          <MenuItem value="workspace">Entire workspace</MenuItem>
          <MenuItem value="blueprint_family">Blueprint family</MenuItem>
          <MenuItem value="context_subtree">Context subtree</MenuItem>
          <MenuItem value="entity">Entity</MenuItem>
        </TextField>
      )}
    </form.Field>
    <form.Subscribe selector={(state: any) => state.values.scope_type}>
      {(scope: ScopeType) => (
        <ScopeTargetField form={form} scope={scope} workspaceId={workspaceId} />
      )}
    </form.Subscribe>
  </>
);

const ScopeTargetField = ({
  form,
  scope,
  workspaceId,
}: {
  form: any;
  scope: ScopeType;
  workspaceId?: string;
}) => {
  const targets = useQuery({
    enabled: scope !== 'workspace',
    queryKey: workspaceQueryKeys.grantTargets(scope),
    queryFn: () => listGrantTargets(scope),
  });
  if (scope === 'workspace')
    return (
      <form.Field name="scope_target_id">
        {(field: any) => (
          <TextField
            fullWidth
            helperText="The configured active workspace"
            label="Workspace"
            slotProps={{ input: { readOnly: true } }}
            value={workspaceId ?? ''}
          />
        )}
      </form.Field>
    );
  const label =
    scope === 'blueprint_family'
      ? 'Blueprint family'
      : scope === 'context_subtree'
        ? 'Context subtree'
        : 'Entity';
  return (
    <>
      {targets.isError && (
        <Alert severity="error">{targets.error.message}</Alert>
      )}
      <form.Field name="scope_target_id">
        {(field: any) => (
          <TextField
            fullWidth
            helperText="Only targets owned by the active workspace are available."
            label={label}
            onChange={(event) => field.handleChange(event.target.value)}
            select
            value={field.state.value}
          >
            {targets.data?.map((target) => (
              <MenuItem key={target.id} value={target.id}>
                {target.label}
              </MenuItem>
            ))}
          </TextField>
        )}
      </form.Field>
    </>
  );
};

export const WorkspaceManagementPage = ({ section }: { section: Section }) => {
  const session = useQuery({
    queryKey: ['auth', 'session'],
    queryFn: currentSession,
  });
  return (
    <Box sx={{ maxWidth: 1000, mx: 'auto', p: 3 }}>
      <Typography variant="h4">Workspace management</Typography>
      <Typography color="text.secondary" sx={{ mt: 1 }}>
        Active workspace: {session.data?.workspace_id ?? 'Loading…'} (configured
        by this deployment)
      </Typography>
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mt: 3 }}>
        {sections
          .filter(
            (item) =>
              (item.section === 'roles' &&
                session.data?.capabilities?.roles_manage) ||
              (item.section === 'tokens' &&
                session.data?.capabilities?.tokens_manage) ||
              ((item.section === 'members' || item.section === 'invitations') &&
                session.data?.capabilities?.members_manage),
          )
          .map((item) => (
            <Button
              component={Link}
              key={item.section}
              to={item.to}
              variant={item.section === section ? 'contained' : 'outlined'}
            >
              {item.label}
            </Button>
          ))}
      </Box>
      {session.isError && (
        <Alert severity="error">{session.error.message}</Alert>
      )}
      {section === 'members' && (
        <Members
          canManage={session.data?.capabilities?.members_manage === true}
          currentUserId={session.data?.user_id}
          workspaceId={session.data?.workspace_id}
        />
      )}
      {section === 'roles' && (
        <Roles canManage={session.data?.capabilities?.roles_manage === true} />
      )}
      {section === 'invitations' && (
        <Invitations
          canManage={session.data?.capabilities?.members_manage === true}
          workspaceId={session.data?.workspace_id}
        />
      )}
      {section === 'tokens' && (
        <Tokens
          canManage={session.data?.capabilities?.tokens_manage === true}
        />
      )}
    </Box>
  );
};

const Members = ({
  canManage,
  currentUserId,
  workspaceId,
}: {
  canManage: boolean;
  currentUserId?: string;
  workspaceId?: string;
}) => {
  const client = useQueryClient();
  const [error, setError] = useState<string>();
  const members = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.members(),
    queryFn: listMembers,
  });
  const roles = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.assignableRoles(),
    queryFn: listAssignableRoles,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: workspaceQueryKeys.members() });
  const form = useForm({
    defaultValues: {
      member_id: '',
      role_id: '',
      scope_type: 'workspace' as ScopeType,
      scope_target_id: workspaceId ?? '',
    },
    onSubmit: async ({ value }) => {
      try {
        const input = ensureActiveScopeTarget(
          {
            role_id: value.role_id,
            scope_type: value.scope_type,
            scope_target_id:
              value.scope_type === 'workspace'
                ? (workspaceId ?? '')
                : value.scope_target_id,
          },
          workspaceId,
          client.getQueryData(
            workspaceQueryKeys.grantTargets(value.scope_type),
          ),
        );
        await grantMemberRole(value.member_id, input);
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error ? reason.message : 'Could not grant role',
        );
      }
    },
  });
  const isOwner = members.data?.some(
    (member) =>
      member.user_id === currentUserId &&
      member.grants.some(
        (grant) =>
          grant.role_code === 'owner' &&
          grant.scope_type === 'workspace' &&
          grant.scope_target_id === workspaceId,
      ),
  );
  if (!canManage) {
    return (
      <Alert severity="error">You are not authorized to manage members.</Alert>
    );
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      {error && <Alert severity="error">{error}</Alert>}
      {members.isError && (
        <Alert severity="error">{members.error.message}</Alert>
      )}
      {roles.isError && <Alert severity="error">{roles.error.message}</Alert>}
      <Paper>
        <List>
          {members.data?.map((member) => (
            <ListItem
              divider
              key={member.id}
              secondaryAction={
                <Stack direction="row" sx={{ gap: 1 }}>
                  <Button
                    onClick={() =>
                      setMemberState(
                        member.id,
                        member.state === 'active' ? 'inactive' : 'active',
                      )
                        .then(refresh)
                        .catch((e) => setError(e.message))
                    }
                  >
                    {member.state === 'active' ? 'Deactivate' : 'Activate'}
                  </Button>
                  {isOwner && (
                    <Button
                      onClick={() =>
                        transferOwnership(member.id)
                          .then(refresh)
                          .catch((e) => setError(e.message))
                      }
                    >
                      Transfer ownership
                    </Button>
                  )}
                </Stack>
              }
            >
              <ListItemText
                primary={member.display_name ?? member.email}
                secondary={`${member.email} · ${member.state}${member.grants.length ? ` · ${member.grants.map((grant) => grant.role_code).join(', ')}` : ''}`}
              />
              {member.grants.map((grant) => (
                <Button
                  key={grant.id}
                  onClick={() =>
                    revokeMemberRole(member.id, grant.id)
                      .then(refresh)
                      .catch((e) => setError(e.message))
                  }
                >
                  Revoke {grant.role_code}
                </Button>
              ))}
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
        <Stack spacing={2}>
          <Typography variant="h6">Add role grant</Typography>
          <form.Field name="member_id">
            {(field) => (
              <TextField
                label="Member"
                onChange={(event) => field.handleChange(event.target.value)}
                select
                value={field.state.value}
              >
                {members.data
                  ?.filter((member) => member.state === 'active')
                  .map((member) => (
                    <MenuItem key={member.id} value={member.id}>
                      {member.email}
                    </MenuItem>
                  ))}
              </TextField>
            )}
          </form.Field>
          <form.Field name="role_id">
            {(field) => (
              <TextField
                label="Role"
                onChange={(event) => field.handleChange(event.target.value)}
                select
                value={field.state.value}
              >
                {roles.data?.map((role) => (
                  <MenuItem key={role.id} value={role.id}>
                    {role.code}
                  </MenuItem>
                ))}
              </TextField>
            )}
          </form.Field>
          <ScopeFields form={form} workspaceId={workspaceId} />
          <Button type="submit" variant="contained">
            Grant role
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};

const Roles = ({ canManage }: { canManage: boolean }) => {
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
          reason instanceof Error ? reason.message : 'Could not create role',
        );
      }
    },
  });
  if (!canManage) {
    return (
      <Alert severity="error">You are not authorized to manage roles.</Alert>
    );
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
                        const code = window.prompt('Role code', role.code);
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
                        const code = window.prompt('New role code');
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
                          'Replacement role ID (required when this role has grants; leave empty otherwise)',
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
                primary={`${role.code}${role.is_system ? ' (fixed)' : ''}`}
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
          <Typography variant="h6">Create custom role</Typography>
          <form.Field name="code">
            {(field) => (
              <TextField
                label="Role code"
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

const Invitations = ({
  canManage,
  workspaceId,
}: {
  canManage: boolean;
  workspaceId?: string;
}) => {
  const client = useQueryClient();
  const [secret, setSecret] = useState<string>();
  const [error, setError] = useState<string>();
  const invitations = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.invitations(),
    queryFn: listInvitations,
  });
  const roles = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.assignableRoles(),
    queryFn: listAssignableRoles,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: workspaceQueryKeys.invitations() });
  const form = useForm({
    defaultValues: {
      email: '',
      role_id: '',
      scope_type: 'workspace' as ScopeType,
      scope_target_id: workspaceId ?? '',
      expires_at: '',
    },
    onSubmit: async ({ value }) => {
      try {
        const expiresAt = new Date(value.expires_at);
        if (Number.isNaN(expiresAt.getTime()) || expiresAt <= new Date()) {
          throw new Error('Invitation expiry must be in the future.');
        }
        const input = ensureActiveScopeTarget(
          {
            role_id: value.role_id,
            scope_type: value.scope_type,
            scope_target_id:
              value.scope_type === 'workspace'
                ? (workspaceId ?? '')
                : value.scope_target_id,
          },
          workspaceId,
          client.getQueryData(
            workspaceQueryKeys.grantTargets(value.scope_type),
          ),
        );
        const result = await createInvitation({
          email: value.email,
          ...input,
          expires_at: expiresAt.toISOString(),
        });
        setSecret(result.secret);
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : 'Could not create invitation',
        );
      }
    },
  });
  if (!canManage) {
    return (
      <Alert severity="error">
        You are not authorized to manage invitations.
      </Alert>
    );
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <SecretDialog onClose={() => setSecret(undefined)} secret={secret} />
      {error && <Alert severity="error">{error}</Alert>}
      {invitations.isError && (
        <Alert severity="error">{invitations.error.message}</Alert>
      )}
      {roles.isError && <Alert severity="error">{roles.error.message}</Alert>}
      <Paper>
        <List>
          {invitations.data?.map((item) => (
            <ListItem
              divider
              key={item.id}
              secondaryAction={
                !item.revoked_at &&
                !item.accepted_at && (
                  <Button
                    color="error"
                    onClick={() =>
                      revokeInvitation(item.id)
                        .then(refresh)
                        .catch((e) => setError(e.message))
                    }
                  >
                    Revoke
                  </Button>
                )
              }
            >
              <ListItemText
                primary={item.invitee_email}
                secondary={`${item.role_code} · expires ${new Date(item.expires_at).toLocaleString()}`}
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
        <Stack spacing={2}>
          <Typography variant="h6">Invite member</Typography>
          <form.Field name="email">
            {(field) => (
              <TextField
                label="Email"
                onChange={(event) => field.handleChange(event.target.value)}
                type="email"
                value={field.state.value}
              />
            )}
          </form.Field>
          <form.Field name="role_id">
            {(field) => (
              <TextField
                label="Role"
                onChange={(event) => field.handleChange(event.target.value)}
                select
                value={field.state.value}
              >
                {roles.data?.map((role) => (
                  <MenuItem key={role.id} value={role.id}>
                    {role.code}
                  </MenuItem>
                ))}
              </TextField>
            )}
          </form.Field>
          <ScopeFields form={form} workspaceId={workspaceId} />
          <form.Field name="expires_at">
            {(field) => (
              <TextField
                slotProps={{ inputLabel: { shrink: true } }}
                label="Expires at"
                onChange={(event) => field.handleChange(event.target.value)}
                type="datetime-local"
                value={field.state.value}
              />
            )}
          </form.Field>
          <Button type="submit" variant="contained">
            Create invitation
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};

const Tokens = ({ canManage }: { canManage: boolean }) => {
  const client = useQueryClient();
  const [secret, setSecret] = useState<string>();
  const [error, setError] = useState<string>();
  const tokens = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.tokens(),
    queryFn: listTokens,
  });
  const permissions = useQuery({
    enabled: canManage,
    queryKey: workspaceQueryKeys.tokenPermissions(),
    queryFn: listTokenPermissions,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: workspaceQueryKeys.tokens() });
  const form = useForm({
    defaultValues: { label: '', permissions: [] as string[], expires_at: '' },
    onSubmit: async ({ value }) => {
      try {
        const expiresAt = value.expires_at
          ? new Date(value.expires_at)
          : undefined;
        if (
          expiresAt &&
          (Number.isNaN(expiresAt.getTime()) || expiresAt <= new Date())
        ) {
          throw new Error('Token expiry must be in the future.');
        }
        const token = await createToken({
          label: value.label,
          permissions: value.permissions,
          ...(expiresAt ? { expires_at: expiresAt.toISOString() } : {}),
        });
        setSecret(token.secret);
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error ? reason.message : 'Could not create token',
        );
      }
    },
  });
  if (!canManage) {
    return (
      <Alert severity="error">You are not authorized to manage tokens.</Alert>
    );
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <SecretDialog onClose={() => setSecret(undefined)} secret={secret} />
      {error && <Alert severity="error">{error}</Alert>}
      {tokens.isError && <Alert severity="error">{tokens.error.message}</Alert>}
      {permissions.isError && (
        <Alert severity="error">{permissions.error.message}</Alert>
      )}
      <Paper>
        <List>
          {tokens.data?.map((token) => (
            <ListItem
              divider
              key={token.id}
              secondaryAction={
                !token.revoked_at && (
                  <Button
                    color="error"
                    onClick={() =>
                      revokeToken(token.id)
                        .then(refresh)
                        .catch((e) => setError(e.message))
                    }
                  >
                    Revoke
                  </Button>
                )
              }
            >
              <ListItemText
                primary={token.label}
                secondary={`${token.permissions.join(', ')} · created ${new Date(token.created_at).toLocaleString()}`}
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
          <Typography variant="h6">Create personal token</Typography>
          <form.Field name="label">
            {(field) => (
              <TextField
                label="Label"
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
                    label={permission.code}
                  />
                ))}
              </>
            )}
          </form.Field>
          <form.Field name="expires_at">
            {(field) => (
              <TextField
                slotProps={{ inputLabel: { shrink: true } }}
                label="Expires at (optional)"
                onChange={(event) => field.handleChange(event.target.value)}
                type="datetime-local"
                value={field.state.value}
              />
            )}
          </form.Field>
          <Button type="submit" variant="contained">
            Create token
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};

export const AcceptInvitationPage = () => {
  const [secret, setSecret] = useState('');
  const [message, setMessage] = useState<string>();
  const submit = async () => {
    try {
      await acceptInvitation(secret);
      setSecret('');
      setMessage('Invitation accepted. You can now use the workspace.');
    } catch (reason) {
      setMessage(
        reason instanceof Error
          ? reason.message
          : 'Could not accept invitation',
      );
    }
  };
  return (
    <Box sx={{ maxWidth: 500, mx: 'auto', p: 3 }}>
      <Typography variant="h4">Accept workspace invitation</Typography>
      <Stack spacing={2} sx={{ mt: 3 }}>
        <TextField
          autoComplete="off"
          label="Invitation secret"
          onChange={(event) => setSecret(event.target.value)}
          type="password"
          value={secret}
        />
        {message && (
          <Alert
            severity={
              message.startsWith('Invitation accepted') ? 'success' : 'error'
            }
          >
            {message}
          </Alert>
        )}
        <Button onClick={submit} variant="contained">
          Accept invitation
        </Button>
      </Stack>
    </Box>
  );
};

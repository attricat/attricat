import { useForm } from '@tanstack/react-form';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { Link, useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Checkbox,
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
import { useTranslation } from 'react-i18next';
import { currentSession } from '../auth/api';
import {
  acceptInvitation,
  createInvitation,
  createWorkspaceUser,
  completeOnboarding,
  createRole,
  duplicateRole,
  ensureActiveScopeTarget,
  grantMemberRole,
  listAssignableRoles,
  listGrantTargets,
  listInvitations,
  listMembers,
  listPermissions,
  listRoles,
  retireRole,
  revokeInvitation,
  revokeMemberRole,
  selectedScopeTarget,
  transferOwnership,
  updateRole,
  type ScopeType,
} from './api';
import { workspaceQueryKeys } from './query-keys';

type Section = 'members' | 'roles' | 'invitations';
const sections: { labelKey: string; section: Section; to: string }[] = [
  {
    labelKey: 'workspace.members',
    section: 'members',
    to: '/manage/workspace/members',
  },
  {
    labelKey: 'workspace.roles',
    section: 'roles',
    to: '/manage/workspace/roles',
  },
  {
    labelKey: 'workspace.invitations',
    section: 'invitations',
    to: '/manage/workspace/invitations',
  },
];

type ScopeFieldsProps = {
  onScopeChange: (scope: ScopeType) => void;
  onScopeTargetChange: (scopeTargetId: string) => void;
  scope: ScopeType;
  scopeTargetId: string;
  workspaceId?: string;
};

const ScopeFields = ({
  onScopeChange,
  onScopeTargetChange,
  scope,
  scopeTargetId,
  workspaceId,
}: ScopeFieldsProps) => {
  const { t } = useTranslation();
  return (
    <>
      <TextField
        fullWidth
        label={t('workspace.scope')}
        onChange={(event) => {
          const nextScope = event.target.value as ScopeType;
          onScopeChange(nextScope);
          onScopeTargetChange(selectedScopeTarget(nextScope, '', workspaceId));
        }}
        select
        value={scope}
      >
        <MenuItem value="workspace">{t('workspace.entireWorkspace')}</MenuItem>
        <MenuItem value="blueprint_family">
          {t('workspace.blueprintFamily')}
        </MenuItem>
        <MenuItem value="context_subtree">
          {t('workspace.contextSubtree')}
        </MenuItem>
        <MenuItem value="entity">{t('workspace.entity')}</MenuItem>
      </TextField>
      <ScopeTargetField
        onScopeTargetChange={onScopeTargetChange}
        scope={scope}
        scopeTargetId={scopeTargetId}
      />
    </>
  );
};

const ScopeTargetField = ({
  onScopeTargetChange,
  scope,
  scopeTargetId,
}: Pick<
  ScopeFieldsProps,
  'onScopeTargetChange' | 'scope' | 'scopeTargetId'
>) => {
  const targets = useQuery({
    enabled: scope !== 'workspace',
    queryKey: workspaceQueryKeys.grantTargets(scope),
    queryFn: () => listGrantTargets(scope),
  });
  if (scope === 'workspace') return null;
  const label =
    scope === 'blueprint_family'
      ? t('workspace.blueprintFamily')
      : scope === 'context_subtree'
        ? t('workspace.contextSubtree')
        : t('workspace.entity');
  return (
    <>
      {targets.isError && (
        <Alert severity="error">{targets.error.message}</Alert>
      )}
      <TextField
        fullWidth
        helperText={t('workspace.ownedTargets')}
        label={label}
        onChange={(event) => onScopeTargetChange(event.target.value)}
        select
        value={scopeTargetId}
      >
        {targets.data?.map((target) => (
          <MenuItem key={target.id} value={target.id}>
            {target.label}
          </MenuItem>
        ))}
      </TextField>
    </>
  );
};

export const WorkspaceManagementPage = ({ section }: { section: Section }) => {
  const { t } = useTranslation();
  const session = useQuery({
    queryKey: ['auth', 'session'],
    queryFn: currentSession,
  });
  return (
    <Box sx={{ maxWidth: 1000, mx: 'auto', p: 3 }}>
      <Typography variant="h4">{t('workspace.title')}</Typography>
      <Typography color="text.secondary" sx={{ mt: 1 }}>
        {t('workspace.active', {
          workspace: session.data?.login_identifier ?? t('workspace.loading'),
        })}
      </Typography>
      <Box sx={{ display: 'flex', flexWrap: 'wrap', gap: 1, mt: 3 }}>
        {sections
          .filter(
            (item) =>
              (item.section === 'roles' &&
                session.data?.capabilities?.roles_manage) ||
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
              {t(item.labelKey)}
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
  const { t } = useTranslation();
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
            scope_target_id: selectedScopeTarget(
              value.scope_type,
              value.scope_target_id,
              workspaceId,
            ),
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
          reason instanceof Error
            ? reason.message
            : t('workspace.grantRoleFailed'),
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
      <Alert severity="error">{t('workspace.notAuthorizedMembers')}</Alert>
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
            <ListItem divider key={member.id}>
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
                  {t('workspace.revoke', { role: grant.role_code })}
                </Button>
              ))}
              <Stack direction="row" sx={{ gap: 1 }}>
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
          <Typography variant="h6">{t('workspace.addRoleGrant')}</Typography>
          <form.Field name="member_id">
            {(field) => (
              <TextField
                label={t('workspace.member')}
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
                label={t('workspace.role')}
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
          <form.Subscribe selector={(state) => state.values}>
            {(values) => (
              <ScopeFields
                onScopeChange={(scope) =>
                  form.setFieldValue('scope_type', scope)
                }
                onScopeTargetChange={(scopeTargetId) =>
                  form.setFieldValue('scope_target_id', scopeTargetId)
                }
                scope={values.scope_type}
                scopeTargetId={values.scope_target_id}
                workspaceId={workspaceId}
              />
            )}
          </form.Subscribe>
          <Button type="submit" variant="contained">
            Grant role
          </Button>
        </Stack>
      </Paper>
    </Stack>
  );
};

const Roles = ({ canManage }: { canManage: boolean }) => {
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

const Invitations = ({
  canManage,
  workspaceId,
}: {
  canManage: boolean;
  workspaceId?: string;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
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
          throw new Error(t('workspace.invitationExpiry'));
        }
        const input = ensureActiveScopeTarget(
          {
            role_id: value.role_id,
            scope_type: value.scope_type,
            scope_target_id: selectedScopeTarget(
              value.scope_type,
              value.scope_target_id,
              workspaceId,
            ),
          },
          workspaceId,
          client.getQueryData(
            workspaceQueryKeys.grantTargets(value.scope_type),
          ),
        );
        await createInvitation({
          email: value.email,
          ...input,
          expires_at: expiresAt.toISOString(),
        });
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : t('workspace.createInvitationFailed'),
        );
      }
    },
  });
  const userForm = useForm({
    defaultValues: {
      email: '',
      display_name: '',
      role_id: '',
      scope_type: 'workspace' as ScopeType,
      scope_target_id: workspaceId ?? '',
      expires_at: '',
    },
    onSubmit: async ({ value }) => {
      try {
        const expiresAt = new Date(value.expires_at);
        if (Number.isNaN(expiresAt.getTime()) || expiresAt <= new Date())
          throw new Error(t('workspace.invitationExpiry'));
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
        await createWorkspaceUser({
          email: value.email,
          display_name: value.display_name || undefined,
          ...input,
          expires_at: expiresAt.toISOString(),
        });
        refresh();
      } catch (reason) {
        setError(
          reason instanceof Error
            ? reason.message
            : t('workspace.createUserFailed'),
        );
      }
    },
  });
  if (!canManage) {
    return (
      <Alert severity="error">{t('workspace.notAuthorizedInvitations')}</Alert>
    );
  }
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
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
                secondary={`${item.role_code} · ${t('workspace.expires', { date: new Date(item.expires_at).toLocaleString() })}`}
              />
            </ListItem>
          ))}
        </List>
      </Paper>
      <Paper
        component="form"
        onSubmit={(event) => {
          event.preventDefault();
          userForm.handleSubmit();
        }}
        sx={{ p: 2 }}
      >
        <Stack spacing={2}>
          <Typography variant="h6">
            {t('workspace.createUserInvite')}
          </Typography>
          <Alert severity="info">{t('workspace.inviteEmailInfo')}</Alert>
          <userForm.Field name="email">
            {(field) => (
              <TextField
                label={t('workspace.email')}
                type="email"
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </userForm.Field>
          <userForm.Field name="display_name">
            {(field) => (
              <TextField
                label={t('workspace.displayNameOptional')}
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </userForm.Field>
          <userForm.Field name="role_id">
            {(field) => (
              <TextField
                label={t('workspace.role')}
                select
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              >
                {roles.data?.map((role) => (
                  <MenuItem key={role.id} value={role.id}>
                    {role.code}
                  </MenuItem>
                ))}
              </TextField>
            )}
          </userForm.Field>
          <userForm.Subscribe selector={(state) => state.values}>
            {(values) => (
              <ScopeFields
                onScopeChange={(scope) =>
                  userForm.setFieldValue('scope_type', scope)
                }
                onScopeTargetChange={(scopeTargetId) =>
                  userForm.setFieldValue('scope_target_id', scopeTargetId)
                }
                scope={values.scope_type}
                scopeTargetId={values.scope_target_id}
                workspaceId={workspaceId}
              />
            )}
          </userForm.Subscribe>
          <userForm.Field name="expires_at">
            {(field) => (
              <TextField
                label={t('workspace.expiresAt')}
                type="datetime-local"
                slotProps={{ inputLabel: { shrink: true } }}
                value={field.state.value}
                onChange={(event) => field.handleChange(event.target.value)}
              />
            )}
          </userForm.Field>
          <Button type="submit" variant="contained">
            Create user and invite
          </Button>
        </Stack>
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
          <Typography variant="h6">{t('workspace.inviteExisting')}</Typography>
          <form.Field name="email">
            {(field) => (
              <TextField
                label={t('workspace.email')}
                onChange={(event) => field.handleChange(event.target.value)}
                type="email"
                value={field.state.value}
              />
            )}
          </form.Field>
          <form.Field name="role_id">
            {(field) => (
              <TextField
                label={t('workspace.role')}
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
          <form.Subscribe selector={(state) => state.values}>
            {(values) => (
              <ScopeFields
                onScopeChange={(scope) =>
                  form.setFieldValue('scope_type', scope)
                }
                onScopeTargetChange={(scopeTargetId) =>
                  form.setFieldValue('scope_target_id', scopeTargetId)
                }
                scope={values.scope_type}
                scopeTargetId={values.scope_target_id}
                workspaceId={workspaceId}
              />
            )}
          </form.Subscribe>
          <form.Field name="expires_at">
            {(field) => (
              <TextField
                slotProps={{ inputLabel: { shrink: true } }}
                label={t('workspace.expiresAt')}
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

export const PasswordSetupPage = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const [invitationSecret, setInvitationSecret] = useState(
    () =>
      new URLSearchParams(window.location.search).get('invitation_secret') ??
      '',
  );
  const [onboardingSecret, setOnboardingSecret] = useState(
    () =>
      new URLSearchParams(window.location.search).get('onboarding_secret') ??
      '',
  );
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [message, setMessage] = useState<string>();
  const submit = async () => {
    try {
      if (password.length < 12)
        throw new Error(t('workspace.passwordTooShort'));
      if (password !== confirmPassword)
        throw new Error(t('workspace.passwordMismatch'));
      await completeOnboarding({
        invitation_secret: invitationSecret,
        onboarding_secret: onboardingSecret,
        password,
      });
      await navigate({ to: '/' });
    } catch (reason) {
      setMessage(
        reason instanceof Error
          ? reason.message
          : t('workspace.onboardingFailed'),
      );
    }
  };
  return (
    <Box sx={{ maxWidth: 500, mx: 'auto', p: 3 }}>
      <Typography variant="h4">{t('workspace.setupTitle')}</Typography>
      <Stack spacing={2} sx={{ mt: 3 }}>
        <TextField
          autoComplete="off"
          label={t('workspace.invitationSecret')}
          onChange={(event) => setInvitationSecret(event.target.value)}
          type="password"
          value={invitationSecret}
        />
        <TextField
          autoComplete="off"
          label={t('workspace.passwordSetupSecret')}
          onChange={(event) => setOnboardingSecret(event.target.value)}
          type="password"
          value={onboardingSecret}
        />
        <TextField
          autoComplete="new-password"
          label={t('workspace.password')}
          onChange={(event) => setPassword(event.target.value)}
          type="password"
          value={password}
        />
        <TextField
          autoComplete="new-password"
          label={t('workspace.confirmPassword')}
          onChange={(event) => setConfirmPassword(event.target.value)}
          type="password"
          value={confirmPassword}
        />
        {message && (
          <Alert
            severity={message.startsWith('Password set') ? 'success' : 'error'}
          >
            {message}
          </Alert>
        )}
        <Button onClick={submit} variant="contained">
          Set password and join workspace
        </Button>
      </Stack>
    </Box>
  );
};

export const AcceptInvitationPage = () => {
  const { t } = useTranslation();
  const [secret, setSecret] = useState(
    () => new URLSearchParams(window.location.search).get('secret') ?? '',
  );
  const [message, setMessage] = useState<string>();
  const submit = async () => {
    try {
      await acceptInvitation(secret);
      setSecret('');
      setMessage(t('workspace.invitationAccepted'));
    } catch (reason) {
      setMessage(
        reason instanceof Error
          ? reason.message
          : t('workspace.acceptInvitationFailed'),
      );
    }
  };
  return (
    <Box sx={{ maxWidth: 500, mx: 'auto', p: 3 }}>
      <Typography variant="h4">{t('workspace.acceptTitle')}</Typography>
      <Stack spacing={2} sx={{ mt: 3 }}>
        <TextField
          autoComplete="off"
          label={t('workspace.invitationSecret')}
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

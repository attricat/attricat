import { useForm } from '@tanstack/react-form';
import { useQuery, useQueryClient } from '@tanstack/react-query';
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
  Paper,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { currentSession } from '../auth/api';
import {
  createToken,
  listTokenPermissions,
  listTokens,
  revokeToken,
} from './api';
import { profileQueryKeys } from './query-keys';

const formatTime = (value: string | null) =>
  value ? new Date(value).toLocaleString() : 'Never';

const tokenPermissionPresets = [
  {
    name: 'Catalog generator',
    description: 'Seed blueprints, contexts, products, and relationships.',
    permissions: [
      'blueprints.read',
      'blueprints.write',
      'blueprints.publish',
      'contexts.read',
      'contexts.write',
      'entities.write',
    ],
  },
  {
    name: 'Read-only catalog',
    description: 'Browse blueprints, contexts, and products without changes.',
    permissions: ['blueprints.read', 'contexts.read', 'entities.read'],
  },
  {
    name: 'Entity importer',
    description: 'Read catalog structure and create or update products.',
    permissions: ['blueprints.read', 'contexts.read', 'entities.write'],
  },
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

const PersonalTokens = ({ canManage }: { canManage: boolean }) => {
  const client = useQueryClient();
  const [secret, setSecret] = useState<string>();
  const [error, setError] = useState<string>();
  const tokens = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokens(),
    queryFn: listTokens,
  });
  const permissions = useQuery({
    enabled: canManage,
    queryKey: profileQueryKeys.tokenPermissions(),
    queryFn: listTokenPermissions,
  });
  const refresh = () =>
    client.invalidateQueries({ queryKey: profileQueryKeys.tokens() });
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
      <Alert severity="info">
        Personal token management is unavailable because you do not have the
        required permission.
      </Alert>
    );
  }
  return (
    <Stack spacing={2}>
      <SecretDialog onClose={() => setSecret(undefined)} secret={secret} />
      {error && <Alert severity="error">{error}</Alert>}
      {tokens.isError && <Alert severity="error">{tokens.error.message}</Alert>}
      {permissions.isError && (
        <Alert severity="error">{permissions.error.message}</Alert>
      )}
      <Paper>
        <List aria-label="Personal API tokens">
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
                        .catch((reason) => setError(reason.message))
                    }
                  >
                    Revoke
                  </Button>
                )
              }
            >
              <ListItemText
                primary={token.label}
                secondary={`Permissions: ${token.permissions.join(', ')} · Created: ${formatTime(token.created_at)} · Last used: ${formatTime(token.last_used_at)} · Expires: ${formatTime(token.expires_at)} · Revoked: ${token.revoked_at ? formatTime(token.revoked_at) : 'No'}`}
              />
            </ListItem>
          ))}
          {tokens.data?.length === 0 && (
            <ListItem>
              <ListItemText primary="No personal API tokens yet." />
            </ListItem>
          )}
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
        <Stack spacing={1}>
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
            {(field) => {
              const availablePermissions = new Set(
                permissions.data?.map((permission) => permission.code),
              );
              return (
                <>
                  <Typography variant="subtitle2">
                    Permission presets
                  </Typography>
                  <Stack direction={{ sm: 'row', xs: 'column' }} spacing={1}>
                    {tokenPermissionPresets.map((preset) => {
                      const unavailable = permissions.data
                        ? preset.permissions.some(
                            (code) => !availablePermissions.has(code),
                          )
                        : false;
                      return (
                        <Box key={preset.name} sx={{ flex: 1 }}>
                          <Button
                            disabled={!permissions.data || unavailable}
                            fullWidth
                            onClick={() =>
                              field.handleChange(preset.permissions)
                            }
                            type="button"
                            variant="outlined"
                          >
                            {preset.name}
                          </Button>
                          <Typography variant="caption">
                            {unavailable
                              ? 'Not available with your current grants.'
                              : preset.description}
                          </Typography>
                        </Box>
                      );
                    })}
                  </Stack>
                  <Typography variant="subtitle2">
                    Custom permissions
                  </Typography>
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
              );
            }}
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

export const ProfilePage = () => {
  const session = useQuery({
    queryKey: ['auth', 'session'],
    queryFn: currentSession,
  });
  const account = session.data;
  return (
    <Box sx={{ maxWidth: 1000, mx: 'auto', p: 3 }}>
      <Typography variant="h4">Profile</Typography>
      {session.isError && (
        <Alert severity="error">{session.error.message}</Alert>
      )}
      <Stack spacing={3} sx={{ mt: 3 }}>
        <Paper sx={{ p: 2 }}>
          <Typography variant="h6">Account details</Typography>
          <Typography>
            Display name: {account?.display_name ?? 'Not set'}
          </Typography>
          <Typography>Email: {account?.email ?? 'Loading…'}</Typography>
          <Typography>User ID: {account?.user_id ?? 'Loading…'}</Typography>
          <Typography>
            Active workspace: {account?.workspace_id ?? 'Loading…'}
          </Typography>
        </Paper>
        <Box id="personal-api-tokens">
          <Typography gutterBottom variant="h5">
            Personal API tokens
          </Typography>
          <PersonalTokens
            canManage={account?.capabilities?.tokens_manage === true}
          />
        </Box>
      </Stack>
    </Box>
  );
};

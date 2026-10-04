import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Autocomplete,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  LinearProgress,
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
  createTeam,
  deleteTeam,
  updateTeam,
  type TeamInput,
} from '../principals/api';
import { userLabel } from '../principals/principal';
import { principalQueryKeys } from '../principals/queryKeys';
import { teamListOptions } from '../principals/queryOptions';
import type { DirectoryUser, Team } from '../principals/schemas';
import { usePrincipalDirectory } from '../principals/usePrincipalDirectory';

type Editing = { team: Team | null } | null;
/** The team keeps its name in the dialog while it fades out after closing. */
type Deleting = { team: Team; open: boolean } | null;

const TeamDialog = ({
  team,
  users,
  onClose,
}: {
  team: Team | null;
  users: DirectoryUser[];
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const save = useMutation({
    mutationFn: (value: TeamInput) =>
      team
        ? updateTeam(team.id, {
            name: value.name,
            member_user_ids: value.member_user_ids,
          })
        : createTeam(value),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: principalQueryKeys.all() });
      onClose();
    },
  });
  const form = useForm({
    defaultValues: {
      code: team?.code ?? '',
      name: team?.name ?? '',
      member_user_ids: team?.member_user_ids ?? [],
    },
    onSubmit: ({ value }) => save.mutateAsync(value).catch(() => undefined),
  });
  // Inactive members stay listed when they already belong to the team.
  const selectable = (selected: readonly string[]) =>
    users.filter((user) => user.active || selected.includes(user.id));
  return (
    <Dialog fullWidth maxWidth="sm" onClose={onClose} open>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void form.handleSubmit();
        }}
      >
        <DialogTitle>
          {t(team ? 'workspace.editTeamTitle' : 'workspace.createTeam')}
        </DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ pt: 1 }}>
            {save.isError && (
              <Alert severity="error">{save.error.message}</Alert>
            )}
            <form.Field name="name">
              {(field) => (
                <TextField
                  label={t('workspace.teamName')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="code">
              {(field) => (
                <TextField
                  disabled={Boolean(team)}
                  helperText={t('workspace.teamCodeHelp')}
                  label={t('workspace.teamCode')}
                  onChange={(event) => field.handleChange(event.target.value)}
                  required
                  value={field.state.value}
                />
              )}
            </form.Field>
            <form.Field name="member_user_ids">
              {(field) => (
                <Autocomplete
                  getOptionLabel={(user) => userLabel(user)}
                  multiple
                  onChange={(_, selected) =>
                    field.handleChange(selected.map((user) => user.id))
                  }
                  options={selectable(field.state.value)}
                  renderInput={(params) => (
                    <TextField {...params} label={t('workspace.teamMembers')} />
                  )}
                  value={users.filter((user) =>
                    field.state.value.includes(user.id),
                  )}
                />
              )}
            </form.Field>
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={onClose}>{t('workspace.cancel')}</Button>
          <form.Subscribe selector={(state) => state.isSubmitting}>
            {(submitting) => (
              <Button disabled={submitting} type="submit" variant="contained">
                {t('workspace.saveTeam')}
              </Button>
            )}
          </form.Subscribe>
        </DialogActions>
      </form>
    </Dialog>
  );
};

/** Teams that user-or-team assignment attributes can reference. */
export const WorkspaceTeamsSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const [editing, setEditing] = useState<Editing>(null);
  const [deleting, setDeleting] = useState<Deleting>(null);
  const teams = useQuery({ ...teamListOptions(), enabled: canManage });
  const directory = usePrincipalDirectory(canManage);
  const remove = useMutation({
    mutationFn: (team: Team) => deleteTeam(team.id),
    onSuccess: async () => {
      await client.invalidateQueries({ queryKey: principalQueryKeys.all() });
      closeDelete();
    },
  });
  const openDelete = (team: Team) => {
    remove.reset();
    setDeleting({ team, open: true });
  };
  const closeDelete = () =>
    setDeleting((current) => current && { ...current, open: false });
  if (!canManage)
    return (
      <Alert severity="error">{t('workspace.notAuthorizedMembers')}</Alert>
    );
  const users = directory.data?.users ?? [];
  const memberNames = (team: Team) =>
    team.member_user_ids
      .map((id) => users.find((user) => user.id === id))
      .filter((user) => user !== undefined)
      .map(userLabel)
      .join(', ');
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <Typography color="text.secondary" variant="body2">
        {t('workspace.teamsDescription')}
      </Typography>
      {teams.isError && <Alert severity="error">{teams.error.message}</Alert>}
      {directory.isError && (
        <Alert severity="error">{directory.error.message}</Alert>
      )}
      {remove.isError && <Alert severity="error">{remove.error.message}</Alert>}
      <Paper>
        {teams.isPending && <LinearProgress aria-label={t('app.loading')} />}
        <List>
          {teams.data?.map((team) => (
            <ListItem
              divider
              key={team.id}
              secondaryAction={
                <Stack direction="row" spacing={1}>
                  <Button
                    aria-label={t('workspace.editNamedTeam', {
                      team: team.name,
                    })}
                    onClick={() => setEditing({ team })}
                    size="small"
                  >
                    {t('workspace.editTeam')}
                  </Button>
                  <Button
                    aria-label={t('workspace.deleteNamedTeam', {
                      team: team.name,
                    })}
                    color="error"
                    onClick={() => openDelete(team)}
                    size="small"
                  >
                    {t('workspace.deleteTeam')}
                  </Button>
                </Stack>
              }
            >
              <ListItemText
                primary={`${team.name} · ${team.code}`}
                secondary={
                  memberNames(team) ||
                  t('workspace.teamMemberCount', {
                    count: team.member_user_ids.length,
                  })
                }
              />
            </ListItem>
          ))}
          {teams.data?.length === 0 && (
            <ListItem>
              <ListItemText primary={t('workspace.noTeams')} />
            </ListItem>
          )}
        </List>
      </Paper>
      <div>
        <Button onClick={() => setEditing({ team: null })} variant="contained">
          {t('workspace.createTeam')}
        </Button>
      </div>
      {editing && (
        <TeamDialog
          onClose={() => setEditing(null)}
          team={editing.team}
          users={users}
        />
      )}
      <Dialog
        onClose={closeDelete}
        open={Boolean(deleting?.open)}
        slotProps={{ transition: { onExited: () => setDeleting(null) } }}
      >
        <DialogTitle>{t('workspace.deleteTeam')}</DialogTitle>
        <DialogContent>
          <DialogContentText>
            {t('workspace.deleteTeamConfirm', { team: deleting?.team.name })}
          </DialogContentText>
        </DialogContent>
        <DialogActions>
          <Button onClick={closeDelete}>{t('workspace.cancel')}</Button>
          <Button
            color="error"
            disabled={remove.isPending}
            onClick={() => deleting && remove.mutate(deleting.team)}
            variant="contained"
          >
            {t('workspace.deleteTeam')}
          </Button>
        </DialogActions>
      </Dialog>
    </Stack>
  );
};

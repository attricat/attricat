import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Autocomplete,
  Button,
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
import { PencilIcon, PlusIcon, Trash2Icon } from 'lucide-react';
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
import { EmptyState } from '../../components/EmptyState';
import { TeamIcon } from '../../components/systemIcons';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { ConfirmDialog } from '../../components/ConfirmDialog';
import { FormDialog } from '../../components/FormDialog';
import { teamCodeMaxLength, teamNameMaxLength } from './constants';

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
    <FormDialog
      error={save.error}
      isPending={save.isPending}
      onClose={onClose}
      onSubmit={() => void form.handleSubmit()}
      submitLabel={t('workspace.saveTeam')}
      title={t(team ? 'workspace.editTeamTitle' : 'workspace.createTeam')}
    >
      <Stack spacing={2} sx={{ pt: 1 }}>
        <form.Field
          name="name"
          validators={{
            // Spaces alone are no name; the API would refuse it.
            onSubmit: ({ value }) =>
              value.trim() ? undefined : t('validation.required'),
          }}
        >
          {(field) => (
            <TextField
              error={field.state.meta.errors.length > 0}
              helperText={field.state.meta.errors[0]}
              label={t('workspace.teamName')}
              onChange={(event) => field.handleChange(event.target.value)}
              required
              slotProps={{ htmlInput: { maxLength: teamNameMaxLength } }}
              value={field.state.value}
            />
          )}
        </form.Field>
        <form.Field name="code">
          {(field) => (
            <TextField
              slotProps={{ htmlInput: { maxLength: teamCodeMaxLength } }}
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
    </FormDialog>
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
    onSuccess: () => {
      // Close before refetching: a slow refetch must not hold the dialog.
      hideDelete();
      void client.invalidateQueries({ queryKey: principalQueryKeys.all() });
    },
  });
  const openDelete = (team: Team) => {
    remove.reset();
    setDeleting({ team, open: true });
  };
  const hideDelete = () =>
    setDeleting((current) => current && { ...current, open: false });
  // The dialog stays open while a delete runs, so its error is shown.
  const closeDelete = () => {
    if (!remove.isPending) hideDelete();
  };
  if (!canManage)
    return <Alert severity="error">{t('workspace.notAuthorizedTeams')}</Alert>;
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
      <ApiErrorAlert error={teams.error} />
      <ApiErrorAlert error={directory.error} />
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
                    startIcon={<PencilIcon />}
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
                    startIcon={<Trash2Icon />}
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
            <ListItem sx={{ display: 'block' }}>
              <EmptyState icon={TeamIcon} title={t('workspace.noTeams')} />
            </ListItem>
          )}
        </List>
      </Paper>
      <div>
        <Button
          onClick={() => setEditing({ team: null })}
          startIcon={<PlusIcon />}
          variant="contained"
        >
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
      <ConfirmDialog
        confirmLabel={t('workspace.deleteTeam')}
        destructive
        error={remove.error}
        isPending={remove.isPending}
        onClose={closeDelete}
        onConfirm={() => deleting?.open && remove.mutate(deleting.team)}
        open={Boolean(deleting?.open)}
        slotProps={{ transition: { onExited: () => setDeleting(null) } }}
        title={t('workspace.deleteTeam')}
      >
        {t('workspace.deleteTeamConfirm', { team: deleting?.team.name })}
      </ConfirmDialog>
    </Stack>
  );
};

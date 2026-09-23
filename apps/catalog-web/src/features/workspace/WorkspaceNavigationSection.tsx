import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  Alert,
  Button,
  Paper,
  Stack,
  TextField,
  Typography,
  MenuItem,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { listEntityBlueprints } from '../entities/api';
import { entityQueryKeys } from '../entities/query-keys';
import {
  listExploreNavigation,
  listRoles,
  updateExploreNavigation,
  type ExploreNavigationEntry,
} from './api';
import { workspaceQueryKeys } from './query-keys';

export const WorkspaceNavigationSection = ({
  canManage,
}: {
  canManage: boolean;
}) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const navigation = useQuery({
    queryKey: workspaceQueryKeys.exploreNavigation(),
    queryFn: listExploreNavigation,
    enabled: canManage,
  });
  const roles = useQuery({
    queryKey: workspaceQueryKeys.roles(),
    queryFn: listRoles,
    enabled: canManage,
  });
  const blueprints = useQuery({
    queryKey: entityQueryKeys.blueprints(),
    queryFn: ({ signal }) => listEntityBlueprints(signal),
    enabled: canManage,
  });
  const [entries, setEntries] = useState<
    ExploreNavigationEntry[] | undefined
  >();
  const displayed = entries ?? navigation.data ?? [];
  const save = useMutation({
    mutationFn: updateExploreNavigation,
    onSuccess: (_, savedEntries) => {
      queryClient.setQueryData(
        workspaceQueryKeys.exploreNavigation(),
        savedEntries,
      );
      void queryClient.invalidateQueries({
        queryKey: workspaceQueryKeys.exploreNavigation(),
      });
      void queryClient.invalidateQueries({
        queryKey: workspaceQueryKeys.sidebarExploreNavigation(),
      });
      setEntries(undefined);
    },
  });
  if (!canManage)
    return (
      <Alert severity="error">{t('workspace.notAuthorizedNavigation')}</Alert>
    );
  return (
    <Stack spacing={2} sx={{ mt: 3 }}>
      <Typography variant="h6">{t('workspace.navigationTitle')}</Typography>
      <Typography color="text.secondary">
        {t('workspace.navigationDescription')}
      </Typography>
      {(navigation.isError || roles.isError || blueprints.isError) && (
        <Alert severity="error">
          {navigation.error?.message ??
            roles.error?.message ??
            blueprints.error?.message}
        </Alert>
      )}
      {displayed.map((entry, index) => (
        <Paper key={entry.blueprint_code} sx={{ p: 2 }}>
          <Stack direction={{ xs: 'column', md: 'row' }} spacing={1.5}>
            <TextField
              disabled={save.isPending}
              label={t('workspace.navigationBlueprint')}
              onChange={(event) =>
                setEntries(
                  displayed.map((item, position) =>
                    position === index
                      ? { ...item, blueprint_code: event.target.value }
                      : item,
                  ),
                )
              }
              select
              value={entry.blueprint_code}
              sx={{ minWidth: 250 }}
            >
              {(blueprints.data ?? []).map((blueprint) => (
                <MenuItem
                  disabled={displayed.some(
                    (item, position) =>
                      position !== index &&
                      item.blueprint_code === blueprint.code,
                  )}
                  key={blueprint.code}
                  value={blueprint.code}
                >
                  {blueprint.name} ({blueprint.code})
                </MenuItem>
              ))}
            </TextField>
            <TextField
              disabled={save.isPending}
              helperText={t('workspace.navigationAvailableRoles', {
                roles: (roles.data ?? []).map((role) => role.code).join(', '),
              })}
              label={t('workspace.navigationVisibleRoles')}
              onChange={(event) =>
                setEntries(
                  displayed.map((item, position) =>
                    position === index
                      ? {
                          ...item,
                          visible_to_role_codes: event.target.value
                            .split(',')
                            .map((code) => code.trim())
                            .filter(Boolean),
                        }
                      : item,
                  ),
                )
              }
              value={entry.visible_to_role_codes.join(', ')}
              sx={{ minWidth: 250 }}
            />
            <Button
              disabled={save.isPending}
              onClick={() =>
                setEntries(
                  displayed.filter((_, position) => position !== index),
                )
              }
            >
              {t('workspace.remove')}
            </Button>
            <Button
              disabled={save.isPending || index === 0}
              onClick={() => {
                const next = [...displayed];
                [next[index - 1], next[index]] = [next[index], next[index - 1]];
                setEntries(next);
              }}
            >
              {t('workspace.moveUp')}
            </Button>
            <Button
              disabled={save.isPending || index === displayed.length - 1}
              onClick={() => {
                const next = [...displayed];
                [next[index + 1], next[index]] = [next[index], next[index + 1]];
                setEntries(next);
              }}
            >
              {t('workspace.moveDown')}
            </Button>
          </Stack>
        </Paper>
      ))}
      <Stack direction="row" spacing={1}>
        <Button
          disabled={save.isPending || !blueprints.data?.length}
          onClick={() => {
            const first = blueprints.data?.find(
              (blueprint) =>
                !displayed.some(
                  (entry) => entry.blueprint_code === blueprint.code,
                ),
            );
            if (first)
              setEntries([
                ...displayed,
                { blueprint_code: first.code, visible_to_role_codes: [] },
              ]);
          }}
        >
          {t('workspace.addShortcut')}
        </Button>
        <Button
          disabled={save.isPending}
          onClick={() => save.mutate(displayed)}
          variant="contained"
        >
          {t('workspace.saveNavigation')}
        </Button>
      </Stack>
      {save.isError && <Alert severity="error">{save.error.message}</Alert>}
    </Stack>
  );
};

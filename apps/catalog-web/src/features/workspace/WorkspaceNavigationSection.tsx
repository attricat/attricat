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
import { ArrowDownIcon, ArrowUpIcon, PlusIcon, Trash2Icon } from 'lucide-react';
import { listRecordBlueprints } from '../records/api';
import { recordQueryKeys } from '../records/queryKeys';
import {
  listExploreNavigation,
  listRoles,
  updateExploreNavigation,
  type ExploreNavigationEntry,
} from './api';
import { navigationSelectMinWidth } from './constants';
import { workspaceQueryKeys } from './queryKeys';
import { lexiconText } from '../lexicon/lexicon';

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
    queryKey: recordQueryKeys.blueprints(),
    queryFn: ({ signal }) => listRecordBlueprints(signal),
    enabled: canManage,
  });
  const [entries, setEntries] = useState<
    ExploreNavigationEntry[] | undefined
  >();
  const [roleDrafts, setRoleDrafts] = useState<Record<string, string>>({});
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
      setRoleDrafts({});
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
              onChange={(event) => {
                setRoleDrafts((drafts) => {
                  const next = { ...drafts };
                  const previous = next[entry.blueprint_code];
                  delete next[entry.blueprint_code];
                  if (previous !== undefined)
                    next[event.target.value] = previous;
                  return next;
                });
                setEntries(
                  displayed.map((item, position) =>
                    position === index
                      ? { ...item, blueprint_code: event.target.value }
                      : item,
                  ),
                );
              }}
              select
              value={entry.blueprint_code}
              sx={{ minWidth: navigationSelectMinWidth }}
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
                  {lexiconText(blueprint.name)} ({blueprint.code})
                </MenuItem>
              ))}
            </TextField>
            <TextField
              disabled={save.isPending}
              helperText={t('workspace.navigationAvailableRoles', {
                roles: (roles.data ?? []).map((role) => role.code).join(', '),
              })}
              label={t('workspace.navigationVisibleRoles')}
              onChange={(event) => {
                const draft = event.target.value;
                setRoleDrafts((current) => ({
                  ...current,
                  [entry.blueprint_code]: draft,
                }));
                setEntries(
                  displayed.map((item, position) =>
                    position === index
                      ? {
                          ...item,
                          visible_to_role_codes: draft
                            .split(',')
                            .map((code) => code.trim())
                            .filter(Boolean),
                        }
                      : item,
                  ),
                );
              }}
              value={
                roleDrafts[entry.blueprint_code] ??
                entry.visible_to_role_codes.join(', ')
              }
              sx={{ minWidth: navigationSelectMinWidth }}
            />
            <Button
              disabled={save.isPending}
              onClick={() => {
                setRoleDrafts((drafts) => {
                  const next = { ...drafts };
                  delete next[entry.blueprint_code];
                  return next;
                });
                setEntries(
                  displayed.filter((_, position) => position !== index),
                );
              }}
              startIcon={<Trash2Icon />}
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
              startIcon={<ArrowUpIcon />}
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
              startIcon={<ArrowDownIcon />}
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
          startIcon={<PlusIcon />}
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

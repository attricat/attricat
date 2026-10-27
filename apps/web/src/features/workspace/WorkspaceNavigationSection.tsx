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
import {
  listExploreNavigation,
  updateExploreNavigation,
  type ExploreNavigationEntry,
} from './api';
import { navigationSelectMinWidth } from './constants';
import { workspaceQueryKeys } from './queryKeys';
import { lexiconText } from '../lexicon/lexicon';
import { ApiErrorAlert } from '../../components/ApiErrorAlert';
import { roleListOptions } from './queryOptions';
import { recordBlueprintsOptions } from '../records/queryOptions';
import { randomUuid } from '../../components/randomUuid';

/** A shortcut with a key that survives changing its blueprint. */
type NavigationRow = ExploreNavigationEntry & { rowKey: string };

const navigationEntry = ({
  blueprint_code,
  visible_to_role_codes,
}: NavigationRow): ExploreNavigationEntry => ({
  blueprint_code,
  visible_to_role_codes,
});

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
    ...roleListOptions(),
    enabled: canManage,
  });
  const blueprints = useQuery({
    ...recordBlueprintsOptions(),
    enabled: canManage,
  });
  const [entries, setEntries] = useState<NavigationRow[] | undefined>();
  // Role text as typed, by row key, so separators survive until saved.
  const [roleDrafts, setRoleDrafts] = useState<Record<string, string>>({});
  // Saved shortcuts are unique by blueprint, which keys them until edited.
  const displayed =
    entries ??
    (navigation.data ?? []).map((entry) => ({
      ...entry,
      rowKey: entry.blueprint_code,
    }));
  const unusedBlueprint = blueprints.data?.find(
    (blueprint) =>
      !displayed.some((entry) => entry.blueprint_code === blueprint.code),
  );
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
      <ApiErrorAlert
        error={navigation.error ?? roles.error ?? blueprints.error}
      />
      {displayed.map((entry, index) => (
        <Paper key={entry.rowKey} sx={{ p: 2 }}>
          <Stack direction={{ xs: 'column', md: 'row' }} spacing={1.5}>
            <TextField
              disabled={save.isPending}
              label={t('workspace.navigationBlueprint')}
              onChange={(event) => {
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
                  [entry.rowKey]: draft,
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
                roleDrafts[entry.rowKey] ??
                entry.visible_to_role_codes.join(', ')
              }
              sx={{ minWidth: navigationSelectMinWidth }}
            />
            <Button
              disabled={save.isPending}
              onClick={() => {
                setRoleDrafts((drafts) => {
                  const next = { ...drafts };
                  delete next[entry.rowKey];
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
          disabled={save.isPending || !unusedBlueprint}
          onClick={() => {
            if (unusedBlueprint)
              setEntries([
                ...displayed,
                {
                  blueprint_code: unusedBlueprint.code,
                  rowKey: randomUuid(),
                  visible_to_role_codes: [],
                },
              ]);
          }}
          startIcon={<PlusIcon />}
        >
          {t('workspace.addShortcut')}
        </Button>
        <Button
          disabled={save.isPending}
          onClick={() => save.mutate(displayed.map(navigationEntry))}
          variant="contained"
        >
          {t('workspace.saveNavigation')}
        </Button>
      </Stack>
      <ApiErrorAlert error={save.error} />
    </Stack>
  );
};

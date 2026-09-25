import { useForm } from '@tanstack/react-form';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
} from '@mui/material';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { inlineExplorerSearch, type ExplorerSearch } from '../explorer/search';
import {
  createSavedView,
  createViewStateLink,
  deleteSavedView,
  getSavedView,
  listSavedViews,
  updateSavedView,
} from './api';
import { savedViewQueryKeys } from './query-keys';
import type { SavedView } from './schemas';

const maximumInlineLinkLength = 1800;

export const SavedSearchActions = ({
  search,
  savedView,
  userId,
}: {
  search: ExplorerSearch;
  savedView?: SavedView;
  userId?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/' });
  const client = useQueryClient();
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');
  const list = useQuery({
    queryKey: savedViewQueryKeys.list(),
    queryFn: ({ signal }) => listSavedViews(signal),
  });
  const originalView = useQuery({
    queryKey: savedViewQueryKeys.detail(search.sourceView ?? '', false),
    queryFn: ({ signal }) =>
      getSavedView(search.sourceView ?? '', false, signal),
    enabled: Boolean(search.sourceView && !savedView),
  });
  const canEdit = Boolean(
    search.sourceView &&
    userId &&
    (savedView ?? originalView.data)?.owner_user_id === userId,
  );
  const isDraft = Boolean(
    search.sourceView &&
    new URLSearchParams(window.location.search).has('sourceView'),
  );
  const save = useMutation({
    mutationFn: (value: {
      name: string;
      description: string;
      visibility: 'private' | 'workspace';
    }) =>
      createSavedView(value.name, value.description, value.visibility, search),
    onSuccess: (view) => {
      void client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      setOpen(false);
      void navigate({ to: '/', search: { savedView: view.id } });
    },
    onError: (error) => setError(error.message),
  });
  const form = useForm({
    defaultValues: {
      name: '',
      description: '',
      visibility: 'private' as 'private' | 'workspace',
    },
    onSubmit: ({ value }) => {
      if (value.name.trim()) save.mutate({ ...value, name: value.name.trim() });
    },
  });
  const [busy, setBusy] = useState(false);
  const update = async () => {
    if (!search.sourceView) return;
    setBusy(true);
    setError('');
    try {
      const original =
        savedView ?? (await getSavedView(search.sourceView, false));
      await updateSavedView(
        original.id,
        original.name ?? '',
        original.description ?? '',
        original.visibility === 'workspace' ? 'workspace' : 'private',
        search,
      );
      await client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      void navigate({ to: '/', search: { savedView: original.id } });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };
  const remove = async () => {
    if (
      !search.sourceView ||
      !window.confirm(t('explorer.deleteSavedSearchConfirm'))
    )
      return;
    setBusy(true);
    setError('');
    try {
      await deleteSavedView(search.sourceView);
      await client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      void navigate({ to: '/', search: { ...search, sourceView: undefined } });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };
  const copyLink = async () => {
    setBusy(true);
    setError('');
    try {
      const inline = inlineExplorerSearch(search);
      const url = new URL(window.location.href);
      if (
        savedView &&
        new URLSearchParams(window.location.search).has('savedView')
      ) {
        url.search = `?savedView=${savedView.id}`;
      } else {
        // TanStack Router's default JSON query serialization matches Explorer's URL format.
        const params = new URLSearchParams();
        Object.entries(inline).forEach(([key, value]) => {
          if (value !== undefined)
            params.set(
              key,
              typeof value === 'object' ? JSON.stringify(value) : String(value),
            );
        });
        url.search = params.toString();
        if (url.href.length > maximumInlineLinkLength) {
          const view = await createViewStateLink(search);
          url.search = `?viewState=${view.id}`;
        }
      }
      await navigator.clipboard.writeText(url.href);
      setNotice(t('explorer.linkCopied'));
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <Stack direction="row" spacing={1} sx={{ my: 2, flexWrap: 'wrap' }}>
        <Button
          disabled={!search.blueprint || busy}
          onClick={() => {
            setError('');
            setOpen(true);
          }}
        >
          {t('explorer.saveSearch')}
        </Button>
        {canEdit && isDraft && (
          <Button disabled={busy} onClick={() => void update()}>
            {t('explorer.saveChanges')}
          </Button>
        )}
        {isDraft && (
          <Button
            disabled={busy}
            onClick={() =>
              void navigate({
                to: '/',
                search: { savedView: search.sourceView },
              })
            }
          >
            {t('explorer.discardSearchChanges')}
          </Button>
        )}
        {canEdit && (
          <Button disabled={busy} color="error" onClick={() => void remove()}>
            {t('explorer.deleteSavedSearch')}
          </Button>
        )}
        <Button
          disabled={!search.blueprint || busy}
          onClick={() => void copyLink()}
        >
          {t('explorer.copyLink')}
        </Button>
        <TextField
          select
          size="small"
          label={t('explorer.savedSearches')}
          value=""
          onChange={(event) =>
            void navigate({
              to: '/',
              search: { savedView: event.target.value },
            })
          }
          sx={{ minWidth: 180 }}
        >
          <MenuItem value="" disabled>
            {t('explorer.savedSearches')}
          </MenuItem>
          {(list.data ?? []).map((view) => (
            <MenuItem key={view.id} value={view.id}>
              {view.name}
            </MenuItem>
          ))}
        </TextField>
      </Stack>
      {notice && (
        <Alert severity="success" onClose={() => setNotice('')}>
          {notice}
        </Alert>
      )}
      {error && (
        <Alert severity="error" onClose={() => setError('')}>
          {error}
        </Alert>
      )}
      {list.isError && <Alert severity="error">{list.error.message}</Alert>}
      <Dialog open={open} onClose={() => setOpen(false)} fullWidth>
        <DialogTitle>{t('explorer.saveSearch')}</DialogTitle>
        <DialogContent>
          <Box
            component="form"
            id="save-search-form"
            onSubmit={(event) => {
              event.preventDefault();
              void form.handleSubmit();
            }}
          >
            <Stack spacing={2} sx={{ pt: 1 }}>
              <form.Field name="name">
                {(field) => (
                  <TextField
                    required
                    label={t('explorer.searchName')}
                    slotProps={{ htmlInput: { maxLength: 120 } }}
                    value={field.state.value}
                    onChange={(event) => field.handleChange(event.target.value)}
                  />
                )}
              </form.Field>
              <form.Field name="description">
                {(field) => (
                  <TextField
                    label={t('explorer.searchDescription')}
                    slotProps={{ htmlInput: { maxLength: 500 } }}
                    value={field.state.value}
                    onChange={(event) => field.handleChange(event.target.value)}
                  />
                )}
              </form.Field>
              <form.Field name="visibility">
                {(field) => (
                  <TextField
                    select
                    label={t('explorer.searchVisibility')}
                    value={field.state.value}
                    onChange={(event) =>
                      field.handleChange(
                        event.target.value as 'private' | 'workspace',
                      )
                    }
                  >
                    <MenuItem value="private">
                      {t('explorer.privateSearch')}
                    </MenuItem>
                    <MenuItem value="workspace">
                      {t('explorer.workspaceSearch')}
                    </MenuItem>
                  </TextField>
                )}
              </form.Field>
              {error && <Alert severity="error">{error}</Alert>}
            </Stack>
          </Box>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setOpen(false)}>{t('common.cancel')}</Button>
          <Button
            type="submit"
            form="save-search-form"
            disabled={save.isPending}
          >
            {t('explorer.saveSearch')}
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
};

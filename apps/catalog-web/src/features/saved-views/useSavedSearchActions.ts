import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import {
  inlineExplorerSearchParams,
  type ExplorerSearch,
} from '../explorer/search';
import {
  createSavedView,
  createViewStateLink,
  deleteSavedView,
  getSavedView,
  listSavedViews,
  updateSavedView,
} from './api';
import {
  MAXIMUM_INLINE_LINK_LENGTH,
  SAVED_VIEW_PARAM,
  SAVED_VIEW_VISIBILITY_PRIVATE,
  SAVED_VIEW_VISIBILITY_WORKSPACE,
  SOURCE_VIEW_PARAM,
  VIEW_STATE_PARAM,
} from './constants';
import { savedViewQueryKeys } from './queryKeys';
import type { SaveSearchValues } from './SaveSearchDialog';
import type { SavedView } from './schemas';

const errorMessage = (cause: unknown) =>
  cause instanceof Error ? cause.message : String(cause);

export const useSavedSearchActions = ({
  savedView,
  search,
  userId,
}: {
  savedView?: SavedView;
  search: ExplorerSearch;
  userId?: string;
}) => {
  const { t } = useTranslation();
  const navigate = useNavigate({ from: '/' });
  const client = useQueryClient();
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState('');
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
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
    new URLSearchParams(window.location.search).has(SOURCE_VIEW_PARAM),
  );
  const save = useMutation({
    mutationFn: (value: SaveSearchValues) =>
      createSavedView(value.name, value.description, value.visibility, search),
    onSuccess: (view) => {
      void client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      setOpen(false);
      void navigate({ to: '/', search: { savedView: view.id } });
    },
    onError: (cause) => setError(cause.message),
  });

  const runBusy = async (action: () => Promise<void>) => {
    setBusy(true);
    setError('');
    try {
      await action();
    } catch (cause) {
      setError(errorMessage(cause));
    } finally {
      setBusy(false);
    }
  };

  const update = () => {
    if (!search.sourceView) return Promise.resolve();
    const sourceView = search.sourceView;
    return runBusy(async () => {
      const original = savedView ?? (await getSavedView(sourceView, false));
      await updateSavedView(
        original.id,
        original.name ?? '',
        original.description ?? '',
        original.visibility === SAVED_VIEW_VISIBILITY_WORKSPACE
          ? SAVED_VIEW_VISIBILITY_WORKSPACE
          : SAVED_VIEW_VISIBILITY_PRIVATE,
        search,
      );
      await client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      void navigate({ to: '/', search: { savedView: original.id } });
    });
  };
  const remove = () => {
    if (
      !search.sourceView ||
      !window.confirm(t('explorer.deleteSavedSearchConfirm'))
    )
      return Promise.resolve();
    const sourceView = search.sourceView;
    return runBusy(async () => {
      await deleteSavedView(sourceView);
      await client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      void navigate({ to: '/', search: { ...search, sourceView: undefined } });
    });
  };
  const copyLink = () =>
    runBusy(async () => {
      const url = new URL(window.location.href);
      if (
        savedView &&
        new URLSearchParams(window.location.search).has(SAVED_VIEW_PARAM)
      ) {
        url.search = `?${SAVED_VIEW_PARAM}=${savedView.id}`;
      } else {
        // TanStack Router's default JSON query serialization matches Explorer's URL format.
        url.search = inlineExplorerSearchParams(search).toString();
        if (url.href.length > MAXIMUM_INLINE_LINK_LENGTH) {
          const view = await createViewStateLink(search);
          url.search = `?${VIEW_STATE_PARAM}=${view.id}`;
        }
      }
      await navigator.clipboard.writeText(url.href);
      setNotice(t('explorer.linkCopied'));
    });

  return {
    busy,
    canEdit,
    copyLink,
    error,
    isDraft,
    list,
    navigate,
    notice,
    open,
    remove,
    save,
    setError,
    setNotice,
    setOpen,
    update,
  };
};

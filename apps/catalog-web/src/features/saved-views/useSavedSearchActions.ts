import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { copyToClipboard } from '../../components/clipboard';
import { useToast } from '../../components/useToast';
import {
  inlineExplorerSearchParams,
  type ExplorerSearch,
} from '../explorer/search';
import {
  createSavedView,
  createViewStateLink,
  deleteSavedView,
  getSavedView,
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
  const { show } = useToast();
  const [saveOpen, setSaveOpen] = useState(false);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState(false);
  const originalView = useQuery({
    queryKey: savedViewQueryKeys.detail(search.sourceView ?? '', false),
    queryFn: ({ signal }) =>
      getSavedView(search.sourceView ?? '', false, signal),
    enabled: Boolean(search.sourceView && !savedView),
  });
  const sourceView = search.sourceView
    ? (savedView ?? originalView.data)
    : undefined;
  const canEdit = Boolean(userId && sourceView?.owner_user_id === userId);
  const isDraft = Boolean(
    search.sourceView &&
    new URLSearchParams(window.location.search).has(SOURCE_VIEW_PARAM),
  );
  const save = useMutation({
    mutationFn: (value: SaveSearchValues) =>
      createSavedView(value.name, value.description, value.visibility, search),
    onSuccess: (view) => {
      void client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      setSaveOpen(false);
      void navigate({ to: '/', search: { savedView: view.id } });
    },
    onError: (cause) => setError(cause.message),
  });

  const runBusy = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } catch (cause) {
      show({ message: errorMessage(cause), severity: 'error' });
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
  const canDelete = (view: SavedView) =>
    Boolean(userId && view.owner_user_id === userId);
  const remove = (id: string) => {
    if (!window.confirm(t('explorer.deleteSavedSearchConfirm')))
      return Promise.resolve();
    return runBusy(async () => {
      await deleteSavedView(id);
      await client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
      if (id === search.sourceView)
        void navigate({
          to: '/',
          search: { ...search, sourceView: undefined },
        });
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
      await copyToClipboard(url.href);
      show({ message: t('explorer.linkCopied'), severity: 'success' });
    });

  const openSaveDialog = () => {
    setError('');
    setSaveOpen(true);
  };
  const openView = (id: string) =>
    navigate({ to: '/', search: { savedView: id } });
  const discard = () =>
    search.sourceView ? openView(search.sourceView) : Promise.resolve();

  return {
    busy,
    canDelete,
    canEdit,
    closeSaveDialog: () => setSaveOpen(false),
    copyLink,
    discard,
    error,
    isDraft,
    openSaveDialog,
    openView,
    remove,
    save,
    saveOpen,
    sourceView,
    update,
  };
};

export type SavedSearchActions = ReturnType<typeof useSavedSearchActions>;

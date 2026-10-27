import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { errorMessage } from '../../api/errors';
import { copyToClipboard } from '../../components/clipboard';
import { useToast } from '../../components/useToast';
import {
  inlineExplorerSearchParams,
  type ExplorerSearch,
} from '../explorer/search';
import { createViewStateLink, deleteSavedView, updateSavedView } from './api';
import {
  MAXIMUM_INLINE_LINK_LENGTH,
  SAVED_VIEW_PARAM,
  SAVED_VIEW_VISIBILITY_PRIVATE,
  SAVED_VIEW_VISIBILITY_WORKSPACE,
  SOURCE_VIEW_PARAM,
  VIEW_STATE_PARAM,
} from './constants';
import { savedViewQueryKeys } from './queryKeys';
import { useCreateSavedView } from './useCreateSavedView';
import type { SavedView } from './schemas';
import { savedViewOptions } from './queryOptions';

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
  const [busy, setBusy] = useState(false);
  const originalView = useQuery({
    ...savedViewOptions(search.sourceView ?? '', false),
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
  const save = useCreateSavedView(() => setSaveOpen(false));

  const runBusy = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } catch (cause) {
      show({
        message: cause instanceof Error ? errorMessage(cause) : String(cause),
        severity: 'error',
      });
    } finally {
      setBusy(false);
    }
  };

  const update = () => {
    if (!search.sourceView) return Promise.resolve();
    const sourceView = search.sourceView;
    return runBusy(async () => {
      const original =
        savedView ??
        (await client.fetchQuery(savedViewOptions(sourceView, false)));
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
  // The caller confirms first; failures are reported as a toast.
  const remove = (id: string) =>
    runBusy(async () => {
      await deleteSavedView(id);
      // Leave the deleted search first, so the refresh below does not refetch
      // it while it is still open and replace the Explorer with a 404.
      if (id === search.sourceView)
        await navigate({
          to: '/',
          search: { ...search, sourceView: undefined },
        });
      client.removeQueries({ queryKey: savedViewQueryKeys.detail(id, false) });
      await client.invalidateQueries({ queryKey: savedViewQueryKeys.all() });
    });
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
    save.reset();
    setSaveOpen(true);
  };
  const openView = (id: string) =>
    navigate({ to: '/', search: { savedView: id } });
  const discard = () =>
    search.sourceView ? openView(search.sourceView) : Promise.resolve();
  // Leaves the saved search intact and returns to a fresh search of its blueprint.
  const clear = () =>
    navigate({
      to: '/',
      search: { blueprint: search.blueprint, locked: search.locked },
    });

  return {
    busy,
    canDelete,
    canEdit,
    clear,
    closeSaveDialog: () => setSaveOpen(false),
    copyLink,
    discard,
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

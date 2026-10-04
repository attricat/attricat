import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useRouterState } from '@tanstack/react-router';
import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { RouterButton } from '../../components/RouterLink';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { draftEditors } from '../drafts/constants';
import { draftResourceIds } from '../drafts/draftStorage';
import {
  useActionDialogStore,
  type OpenActionDialog,
} from './actionDialogStore';
import { extensionLoadingIndicatorSize, selectionSources } from './constants';
import { ExtensionFrame } from './ExtensionFrame';
import { useExtensionRuntime } from './useExtensionRuntime';

/**
 * Host-managed dialog for an extension's selection actions. It is mounted once
 * in the application shell, so it outlives the menu, row or selection toolbar
 * that opened it. The selection is captured when it opens and never changes.
 */
export const ExtensionActionDialogHost = () => {
  const dialog = useActionDialogStore((state) => state.dialog);
  const close = useActionDialogStore((state) => state.close);
  if (!dialog) return null;
  return <ActionDialog dialog={dialog} key={dialog.key} onClose={close} />;
};

const ActionDialog = ({
  dialog: { context, extensionId },
  onClose,
}: {
  dialog: OpenActionDialog;
  onClose: () => void;
}) => {
  const { t } = useTranslation();
  const pathname = useRouterState({
    select: (state) => state.location.pathname,
  });
  const [openedOn] = useState(pathname);
  // Navigating away closes the dialog. A run that was already started keeps
  // running; closing before submission starts nothing.
  useEffect(() => {
    if (pathname !== openedOn) onClose();
  }, [onClose, openedOn, pathname]);
  // MUI restores focus to the opener when it still exists. A row menu or
  // selection toolbar may have unmounted meanwhile; then focus the page's main
  // landmark instead of losing it to the document body.
  const [opener] = useState(() => document.activeElement);
  useEffect(
    () => () => {
      window.requestAnimationFrame(() => {
        if (opener?.isConnected) return;
        const main = document.querySelector<HTMLElement>('main');
        if (!main) return;
        if (!main.hasAttribute('tabindex')) main.tabIndex = -1;
        main.focus();
      });
    },
    [opener],
  );
  const runtime = useExtensionRuntime({
    blueprintId: context.blueprint_id,
    blueprintVersion: context.blueprint_version,
  });
  const contribution = runtime.data?.find(
    (item) =>
      item.extension_id === extensionId &&
      item.kind === 'dialog' &&
      item.outlet === 'action_dialog',
  );
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const workspaceId = session.data?.workspace_id;
  const userId = session.data?.user_id;
  // Read once per opening: the dialog describes what the action will use.
  const unsavedEntityIds = useMemo(() => {
    if (!workspaceId || !userId) return [];
    const drafts = draftResourceIds({
      workspaceId,
      userId,
      editor: draftEditors.entityEdit,
    });
    return context.entity_ids.filter((id) => drafts.has(id));
  }, [context.entity_ids, userId, workspaceId]);
  const count = context.entity_ids.length;
  const title = contribution?.title ?? t('extensions.actionDialogTitle');

  return (
    <Dialog fullWidth maxWidth="sm" onClose={onClose} open>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        <Stack spacing={3}>
          <Typography color="text.secondary" variant="body2">
            {context.selection_source === selectionSources.explorerSelection
              ? t('extensions.actionDialogSelection', { count })
              : t('extensions.actionDialogEntity')}{' '}
            {t('extensions.actionDialogSavedData')}
          </Typography>
          {unsavedEntityIds.length > 0 && (
            <Alert
              action={
                unsavedEntityIds.length === 1 ? (
                  <RouterButton
                    color="inherit"
                    onClick={onClose}
                    params={{ entityId: unsavedEntityIds[0] }}
                    size="small"
                    to="/entities/$entityId/edit"
                  >
                    {t('extensions.actionDialogOpenEditor')}
                  </RouterButton>
                ) : undefined
              }
              severity="warning"
            >
              {t('extensions.actionDialogUnsaved', {
                count: unsavedEntityIds.length,
              })}
            </Alert>
          )}
          {runtime.isPending ? (
            <Box
              aria-live="polite"
              role="status"
              sx={{ display: 'flex', justifyContent: 'center', py: 2 }}
            >
              <CircularProgress
                aria-label={t('extensions.loadingContent')}
                enableTrackSlot
                size={extensionLoadingIndicatorSize}
              />
            </Box>
          ) : contribution ? (
            <ExtensionFrame context={context} contribution={contribution} />
          ) : (
            <Alert role="status" severity="warning">
              {t('extensions.contentLoadFailed')}
            </Alert>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>{t('common.close')}</Button>
      </DialogActions>
    </Dialog>
  );
};

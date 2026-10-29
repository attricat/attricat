import { useQueryClient, type QueryClient } from '@tanstack/react-query';
import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { toast } from '../../components/toast';
import { recordQueryKeys } from '../records/queryKeys';
import { useActionDialogStore } from './actionDialogStore';
import {
  actionContext,
  actionResultSchema,
  type ActionContext,
  type ActionTarget,
} from './actions';
import { extensionCommand, type ExtensionContribution } from './api';
import type { ActionOutlet } from './constants';
import { refreshRecord } from './refreshRecord';

/** Reloads host views of the records an action applied to, and result lists. */
const refreshActionTargets = async (
  client: QueryClient,
  context: ActionContext,
) => {
  await Promise.all([
    ...context.record_ids.map((recordId) => refreshRecord(client, recordId)),
    client.invalidateQueries({ queryKey: recordQueryKeys.searches() }),
  ]);
};

/**
 * Runs a host-rendered action: opens the extension dialog it names, or calls
 * its server command and applies the command's notification and refresh.
 * The context is built here from the host's own target, never by the extension.
 */
export const useInvokeExtensionAction = () => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const openDialog = useActionDialogStore((state) => state.open);
  return useCallback(
    async (
      action: ExtensionContribution,
      outlet: ActionOutlet,
      target: ActionTarget,
    ) => {
      const invoke = action.invoke;
      if (!invoke) return;
      const context = actionContext(action, outlet, target);
      if ('dialog' in invoke) {
        openDialog({
          extensionId: action.extension_id,
          dialogId: invoke.dialog,
          context,
        });
        return;
      }
      try {
        const response = await extensionCommand(
          action.extension_id,
          action.id,
          {
            release_id: action.release_id,
            command_id: invoke.command,
            payload: context,
          },
        );
        const result = actionResultSchema.safeParse(response);
        if (!result.success) return;
        if (result.data.notification) toast.show(result.data.notification);
        if (result.data.refresh)
          await refreshActionTargets(queryClient, context);
      } catch {
        toast.error(
          t('extensions.actionFailed', { action: action.title ?? action.id }),
        );
      }
    },
    [openDialog, queryClient, t],
  );
};

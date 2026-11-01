import { useQueryClient, type QueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { errorMessage } from '../../api/errors';
import { randomUuid } from '../../components/randomUuid';
import { toast } from '../../components/toast';
import {
  extensionRunArtifactUrl,
  getExtensionRun,
  startExtensionRun,
  uploadExtensionRunInput,
} from '../extension-runs/api';
import { extensionRunScopes } from '../extension-runs/constants';
import { notifyRunStarted } from '../extension-runs/runNotifications';
import { recordQueryKeys } from '../records/queryKeys';
import { useActionConfirmStore } from './actionConfirmStore';
import { useActionDialogStore } from './actionDialogStore';
import {
  actionContext,
  actionResultSchema,
  type ActionContext,
  type ActionTarget,
} from './actions';
import { extensionCommand, type ExtensionContribution } from './api';
import { pickFile } from './pickFile';
import { contributionKinds, type ActionOutlet } from './constants';
import { contributionTitle } from './contributionTitle';
import { refreshRecord } from './refreshRecord';
import { downloadArtifact } from './useExtensionFrame';
import { useExtensionRuntime } from './useExtensionRuntime';

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
 * Runs a host-rendered action: asks for its confirmation, then opens the
 * dialog it names, starts its operation, or calls its command and applies
 * the command's result. The context is built here from the host's own
 * target, never by the extension.
 */
export const useInvokeExtensionAction = () => {
  const { i18n, t } = useTranslation();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const openDialog = useActionDialogStore((state) => state.open);
  const askConfirmation = useActionConfirmStore((state) => state.ask);
  // Route results name another contribution of the same extension.
  const runtime = useExtensionRuntime();
  return useCallback(
    async (
      action: ExtensionContribution,
      outlet: ActionOutlet,
      target: ActionTarget,
    ) => {
      const invoke = action.invoke;
      if (!invoke) return;
      const title = contributionTitle(action, i18n.language);
      if (
        action.confirm &&
        !(await askConfirmation({
          title,
          message: action.confirm.message,
          destructive: action.confirm.destructive ?? false,
        }))
      )
        return;
      const context = actionContext(action, outlet, target, i18n.language);
      if ('dialog' in invoke) {
        openDialog({
          extensionId: action.extension_id,
          dialogId: invoke.dialog,
          context,
        });
        return;
      }
      if ('operation' in invoke) {
        const { accepts } = action;
        const file = accepts.length ? await pickFile(accepts) : undefined;
        if (accepts.length && !file) return;
        try {
          const upload = file
            ? await uploadExtensionRunInput(
                action.extension_id,
                action.id,
                file,
              )
            : undefined;
          const { run_id } = await startExtensionRun(
            action.extension_id,
            action.id,
            {
              release_id: action.release_id,
              operation_id: invoke.operation.id,
              input: (invoke.operation.input ?? {}) as Record<string, unknown>,
              idempotency_key: randomUuid(),
              selection: {
                blueprint_id: context.blueprint_id,
                blueprint_version: context.blueprint_version,
                context_id: context.context_id,
                record_ids: context.record_ids,
              },
              ...(upload && { input_file_id: upload.file.id }),
            },
          );
          notifyRunStarted(run_id, navigate, context.record_ids);
        } catch (error) {
          // A person can act on why their file was refused.
          toast.error(
            file && error instanceof Error
              ? errorMessage(error)
              : t('extensions.actionFailed', { action: title }),
          );
        }
        return;
      }
      // Menus close on choosing, so a toast shows the command is running.
      const progress = toast.show({
        autoHideDuration: null,
        message: t('extensions.actionRunning', { action: title }),
      });
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
        toast.dismiss(progress);
        const result = actionResultSchema.safeParse(response);
        if (!result.success) return;
        const { notification, refresh, navigate: to, download } = result.data;
        if (notification) toast.show(notification);
        if (refresh) await refreshActionTargets(queryClient, context);
        if (download) {
          const run = await getExtensionRun(
            download.run_id,
            extensionRunScopes.own,
          );
          if (
            run.extension_id === action.extension_id &&
            run.artifacts.some(
              (artifact) => artifact.id === download.artifact_id,
            )
          )
            downloadArtifact(
              extensionRunArtifactUrl(
                download.run_id,
                download.artifact_id,
                extensionRunScopes.own,
              ),
            );
        }
        if (result.data.open_dialog)
          openDialog({
            extensionId: action.extension_id,
            dialogId: result.data.open_dialog,
            context,
          });
        if (to && 'record_id' in to)
          await navigate({
            to: '/records/$recordId',
            params: { recordId: to.record_id },
          });
        else if (
          to &&
          runtime.data?.some(
            (item) =>
              item.extension_id === action.extension_id &&
              item.id === to.route &&
              item.kind === contributionKinds.route,
          )
        )
          await navigate({
            to: '/extensions/$extensionId/$contributionId',
            params: {
              extensionId: action.extension_id,
              contributionId: to.route,
            },
          });
      } catch {
        toast.dismiss(progress);
        toast.error(t('extensions.actionFailed', { action: title }));
      }
    },
    [
      askConfirmation,
      i18n.language,
      navigate,
      openDialog,
      queryClient,
      runtime.data,
      t,
    ],
  );
};

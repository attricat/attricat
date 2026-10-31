import { Alert, Box, Collapse } from '@mui/material';
import { useTheme } from '@mui/material';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import {
  Component,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  useSyncExternalStore,
  type ErrorInfo,
  type ReactNode,
} from 'react';
import { useTranslation } from 'react-i18next';
import { notifyRunStarted } from '../../extension-runs/runNotifications';
import { useActionDialogStore } from '../actionDialogStore';
import type { ExtensionContribution } from '../api';
import {
  defaultExtensionFrameHeight,
  extensionFramePlaceholders,
  type ExtensionFramePlaceholder,
} from '../constants';
import { useContributionTitle } from '../contributionTitle';
import type { BrokerDependencies } from '../extensionBroker';
import { ExtensionFrameLoading } from '../ExtensionFrameLoading';
import { downloadArtifact } from '../useExtensionFrame';
import { useExtensionRuntime } from '../useExtensionRuntime';
import {
  nativeMountStates,
  nativeUiErrorCodes,
  type NativeUiErrorCode,
} from './constants';
import {
  nativeHostFor,
  type NativeExtensionHost,
  type NativeMountInit,
} from './nativeRuntime';
import {
  NativeFieldLabelContext,
  type NativeFieldLabel,
} from './nativeContexts';
import { NativeTree } from './NativeTree';
import { drawnChildren, type NativeReceiver } from './receiver';
import { ROOT_ID } from '@remote-dom/core';

type Props = {
  contribution: ExtensionContribution;
  context?: Record<string, unknown>;
  /** Receives why the contribution failed. */
  onFailure?: (code: NativeUiErrorCode) => void;
  onReady?: () => void;
  /** An attribute editor's value proposals. */
  onPropose?: (value: unknown) => void;
  placeholder?: ExtensionFramePlaceholder;
  /** Names what the contribution belongs to; it collapses with the content. */
  heading?: ReactNode;
  /** The host field an attribute editor fills. */
  fieldLabel?: NativeFieldLabel;
};

const unsubscribed = () => {};

/** Whether a mounted tree draws anything at its root. */
const useDrawsSomething = (receiver: NativeReceiver | undefined) =>
  useSyncExternalStore(
    useCallback(
      (listener: () => void) =>
        receiver ? receiver.subscribe(ROOT_ID, listener) : unsubscribed,
      [receiver],
    ),
    () => (receiver ? drawnChildren(receiver.root).children.length > 0 : false),
  );

const emptyContext: Record<string, unknown> = {};

/** Fails one mount, never the page, when drawing its tree throws. */
class NativeErrorBoundary extends Component<
  { children: ReactNode; onError: () => void },
  { failed: boolean }
> {
  state = { failed: false };

  static getDerivedStateFromError = () => ({ failed: true });

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error('Could not draw native extension UI', error, info);
    this.props.onError();
  }

  render() {
    return this.state.failed ? null : this.props.children;
  }
}

/**
 * Draws one native contribution: the extension's Worker composes UI kit
 * elements and the host validates and draws them here, inside the outlet's
 * box. Context, locale and theme changes reach the mounted component as
 * updates, so its state survives them.
 */
export const NativeUi = ({
  contribution,
  context = emptyContext,
  onFailure,
  onReady,
  onPropose,
  placeholder = extensionFramePlaceholders.skeleton,
  heading,
  fieldLabel,
}: Props) => {
  const { t, i18n } = useTranslation();
  const label = useContributionTitle()(contribution);
  const runtime = useExtensionRuntime();
  const host = nativeHostFor(contribution, runtime.data ?? [contribution]);
  const colorMode = useTheme().palette.mode;
  const contextKey = JSON.stringify(context);
  const init = useMemo<NativeMountInit>(
    () => ({
      context: JSON.parse(contextKey) as Record<string, unknown>,
      configuration: contribution.configuration,
      capabilities: contribution.capabilities,
      locale: i18n.language,
      theme: { color_mode: colorMode },
      ui_kit: contribution.ui_kit ?? '',
    }),
    [
      contextKey,
      contribution.configuration,
      contribution.capabilities,
      contribution.ui_kit,
      i18n.language,
      colorMode,
    ],
  );
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const initRef = useRef(init);
  const dependencies = useMemo<BrokerDependencies>(
    () => ({
      contribution,
      initialContext: init.context,
      currentContext: () => initRef.current.context,
      navigateToRecord: (recordId, tab) =>
        navigate({
          to: '/records/$recordId',
          params: { recordId },
          search: { tab },
        }),
      queryClient,
      closeActionDialog: useActionDialogStore.getState().close,
      onRunStarted: (runId, recordIds) =>
        notifyRunStarted(runId, navigate, recordIds),
      downloadArtifact,
    }),
    // The initial context scopes blueprint reads for the mount's lifetime.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [contribution, navigate, queryClient],
  );
  const [mount, setMount] = useState<{
    host: NativeExtensionHost;
    mountId: string;
    receiver: NativeReceiver;
  }>();
  const callbacks = useRef({ dependencies, onPropose });
  useEffect(() => {
    callbacks.current = { dependencies, onPropose };
  }, [dependencies, onPropose]);

  // A mount lasts while its Worker host and what was sent with it hold.
  const mountKey = `${host.key}:${contribution.id}:${JSON.stringify([
    contribution.configuration,
    contribution.capabilities,
  ])}`;
  useEffect(() => {
    const created = host.mount(
      contribution,
      initRef.current,
      callbacks.current.dependencies,
      (value) => callbacks.current.onPropose?.(value),
    );
    setMount({ host, ...created });
    return () => host.unmount(created.mountId);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [mountKey]);

  useEffect(() => {
    initRef.current = init;
    if (mount) mount.host.update(mount.mountId, init);
  }, [init, mount]);
  useEffect(() => {
    if (mount)
      mount.host.bind(mount.mountId, dependencies, (value) =>
        callbacks.current.onPropose?.(value),
      );
  }, [dependencies, mount]);

  const snapshot = useSyncExternalStore(
    (listener) => host.subscribe(listener),
    () => {
      const current =
        mount?.host === host ? host.snapshot(mount.mountId) : undefined;
      return `${current?.state ?? nativeMountStates.starting}:${current?.error ?? ''}`;
    },
  );
  const [state, errorCode] = snapshot.split(':') as [
    string,
    NativeUiErrorCode | '',
  ];
  const [renderFailed, setRenderFailed] = useState(false);
  const drawsSomething = useDrawsSomething(
    mount?.host === host ? mount.receiver : undefined,
  );
  const failed = state === nativeMountStates.failed || renderFailed;
  const ready = state === nativeMountStates.ready && !failed;

  const reported = useRef<string>(undefined);
  useEffect(() => {
    const outcome = failed ? 'failed' : ready ? 'ready' : undefined;
    if (!outcome || reported.current === outcome) return;
    reported.current = outcome;
    if (failed)
      onFailure?.(
        renderFailed
          ? nativeUiErrorCodes.renderFailed
          : errorCode || nativeUiErrorCodes.startFailed,
      );
    else onReady?.();
  }, [failed, ready, onFailure, onReady, renderFailed, errorCode]);

  if (failed)
    return (
      <Alert role="status" severity="warning">
        {t(
          `extensions.native.errors.${renderFailed ? nativeUiErrorCodes.renderFailed : errorCode || nativeUiErrorCodes.startFailed}`,
          {
            extension: contribution.extension_name,
          },
        )}
      </Alert>
    );
  const tree = mount && mount.host === host && (
    <Box
      aria-label={label}
      role="region"
      // Native UI draws only inside the outlet's box.
      sx={{ contain: 'layout', minWidth: 0 }}
    >
      {heading}
      <NativeErrorBoundary onError={() => setRenderFailed(true)}>
        <NativeFieldLabelContext.Provider value={fieldLabel}>
          <NativeTree receiver={mount.receiver} />
        </NativeFieldLabelContext.Provider>
      </NativeErrorBoundary>
    </Box>
  );
  // An inline outlet takes no space until the extension draws something.
  if (placeholder === extensionFramePlaceholders.none)
    return <Collapse in={ready && drawsSomething}>{tree}</Collapse>;
  return (
    <Box
      sx={{
        minHeight: ready ? 0 : defaultExtensionFrameHeight,
        position: 'relative',
      }}
    >
      {ready && tree}
      {!ready && <ExtensionFrameLoading />}
    </Box>
  );
};

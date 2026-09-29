import { useTheme } from '@mui/material';
import { useQueryClient } from '@tanstack/react-query';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { getExtensionArtifact, type ExtensionContribution } from './api';
import {
  defaultExtensionFrameHeight,
  extensionMessageTypes,
  extensionStartTimeout,
  maximumExtensionFrameHeight,
} from './constants';
import { handleBrokerRequest, postBrokerResponse } from './extensionBroker';

type FrameContext = Record<string, unknown>;

/** Translation keys for the host-rendered frame failure states. */
type FrameErrorKey =
  | 'extensions.contentLoadFailed'
  | 'extensions.startFailed'
  | 'extensions.startTimedOut';

type UseExtensionFrameOptions = {
  contribution: ExtensionContribution;
  context: FrameContext;
  onContentHeight?: (height: number) => void;
  onFailure?: () => void;
  onReady?: () => void;
};

const clampFrameHeight = (height: number) =>
  Math.min(Math.max(0, height), maximumExtensionFrameHeight);

/**
 * Owns one extension frame session: it waits for the frame document to load,
 * sends the artifact over a fresh MessageChannel, and brokers frame requests.
 */
export const useExtensionFrame = ({
  contribution,
  context,
  onContentHeight,
  onFailure,
  onReady,
}: UseExtensionFrameOptions) => {
  const iframeRef = useRef<HTMLIFrameElement>(null);
  const portRef = useRef<MessagePort | undefined>(undefined);
  const contextKey = JSON.stringify(context);
  const contextRef = useRef(contextKey);
  // Callbacks are read through refs so a parent re-render with new inline
  // callbacks never restarts the frame or calls a stale callback.
  const callbacksRef = useRef({ onContentHeight, onFailure, onReady });
  useEffect(() => {
    callbacksRef.current = { onContentHeight, onFailure, onReady };
  }, [onContentHeight, onFailure, onReady]);
  useEffect(() => {
    contextRef.current = contextKey;
  }, [contextKey]);
  // The color mode is delivered through the port, so switching it never
  // restarts a frame. Every contribution receives it.
  const colorMode = useTheme().palette.mode;
  const colorModeRef = useRef(colorMode);
  useEffect(() => {
    colorModeRef.current = colorMode;
  }, [colorMode]);
  const [errorKey, setErrorKey] = useState<FrameErrorKey>();
  const [ready, setReady] = useState(false);
  const [height, setHeight] = useState(defaultExtensionFrameHeight);
  const [loadedFrame, setLoadedFrame] = useState<string>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  // Context changes are delivered through the broker. A contribution keeps its
  // frame and subscriptions until its release changes or it is unmounted.
  const frameKey = `${contribution.extension_id}:${contribution.id}:${contribution.release_id}`;

  useEffect(() => {
    if (loadedFrame !== frameKey) return;
    const readContext = () => JSON.parse(contextRef.current) as FrameContext;
    const frameContext = readContext();
    let disposed = false;
    let port: MessagePort | undefined;
    const fail = (key: FrameErrorKey) => {
      callbacksRef.current.onFailure?.();
      setErrorKey(key);
    };
    const timer = window.setTimeout(() => {
      if (!disposed) fail('extensions.startTimedOut');
    }, extensionStartTimeout);
    const start = async () => {
      try {
        const artifact = await getExtensionArtifact(
          contribution.extension_id,
          contribution.id,
        );
        if (disposed || !iframeRef.current?.contentWindow) return;
        const channel = new MessageChannel();
        const hostPort = channel.port1;
        port = hostPort;
        portRef.current = hostPort;
        hostPort.onmessage = async ({ data }) => {
          if (!data || typeof data !== 'object') return;
          if (data.type === extensionMessageTypes.ready) {
            window.clearTimeout(timer);
            callbacksRef.current.onReady?.();
            setReady(true);
            return;
          }
          if (data.type === extensionMessageTypes.error) {
            fail('extensions.startFailed');
            return;
          }
          if (
            data.type === extensionMessageTypes.resize &&
            typeof data.height === 'number' &&
            Number.isFinite(data.height)
          ) {
            const nextHeight = clampFrameHeight(data.height);
            setHeight(nextHeight);
            callbacksRef.current.onContentHeight?.(nextHeight);
            return;
          }
          if (
            data.type !== extensionMessageTypes.request ||
            typeof data.id !== 'string'
          )
            return;
          try {
            const result = await handleBrokerRequest(
              { method: data.method, payload: data.payload },
              {
                contribution,
                initialContext: frameContext,
                currentContext: readContext,
                navigateToEntity: (entityId) =>
                  navigate({
                    to: '/entities/$entityId',
                    params: { entityId },
                  }),
                queryClient,
              },
            );
            postBrokerResponse(hostPort, data.id, { ok: true, data: result });
          } catch {
            postBrokerResponse(hostPort, data.id, { ok: false });
          }
        };
        iframeRef.current.contentWindow.postMessage(
          {
            type: extensionMessageTypes.init,
            artifact,
            configuration: contribution.configuration,
            capabilities: contribution.capabilities,
            context: frameContext,
            theme: { color_mode: colorModeRef.current },
          },
          '*',
          [channel.port2],
        );
      } catch {
        if (!disposed) fail('extensions.contentLoadFailed');
      }
    };
    void start();
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      port?.postMessage({ type: extensionMessageTypes.shutdown });
      port?.close();
      if (portRef.current === port) portRef.current = undefined;
    };
  }, [contribution, frameKey, loadedFrame, navigate, queryClient]);

  useEffect(() => {
    if (!ready) return;
    // Only the parent-owned port can update context; the opaque frame cannot
    // forge this versioned message or choose another contribution context.
    portRef.current?.postMessage({
      type: extensionMessageTypes.contextUpdate,
      context: JSON.parse(contextKey) as FrameContext,
    });
  }, [contextKey, ready]);

  useEffect(() => {
    if (!ready) return;
    portRef.current?.postMessage({
      type: extensionMessageTypes.themeUpdate,
      theme: { color_mode: colorMode },
    });
  }, [colorMode, ready]);

  const handleFrameLoad = () => {
    setErrorKey(undefined);
    setHeight(defaultExtensionFrameHeight);
    setReady(false);
    setLoadedFrame(frameKey);
  };

  return { errorKey, frameKey, handleFrameLoad, height, iframeRef, ready };
};

import { Alert, Box, Skeleton, Typography } from '@mui/material';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { z } from 'zod';
import { toast } from '../../components/toast';
import { apiFetch } from '../../api/fetch';
import {
  defaultExtensionFrameHeight,
  extensionStartTimeout,
  maximumExtensionFrameHeight,
  maximumExtensionRequestBytes,
  maximumExtensionResponseBytes,
  maximumExtensionStorageKeyBytes,
} from './constants';
import {
  extensionCommand,
  extensionCommandRequestSchema,
  extensionStorage,
  extensionStorageRequestSchema,
  getExtensionArtifact,
  type ExtensionContribution,
} from './api';

const notificationSchema = z
  .object({
    message: z.string().trim().min(1).max(512),
    severity: z.enum(['success', 'info', 'warning', 'error']).optional(),
  })
  .strict();
const navigationSchema = z.object({ entity_id: z.uuid() }).strict();
const catalogReadSchema = z
  .object({
    // Components may read current entity data or the exact blueprint revision
    // named by the blueprint-configuration outlet. They cannot supply methods,
    // query strings, arbitrary URLs, or credentials.
    path: z
      .string()
      .regex(
        /^\/api\/(?:entities|v1\/entities\/[0-9a-f-]{36}|blueprints\/[0-9a-f-]{36}\/versions\/[1-9][0-9]*)$/,
      ),
  })
  .strict();

const validStorageKey = (key: string) =>
  key.length > 0 &&
  key === key.trim() &&
  new TextEncoder().encode(key).length <= maximumExtensionStorageKeyBytes &&
  ![...key].some((character) => {
    const code = character.charCodeAt(0);
    return code <= 0x1f || (code >= 0x7f && code <= 0x9f);
  });
const validateStorageRequest = (payload: unknown) => {
  const request = extensionStorageRequestSchema.parse(payload);
  const keys = [
    'key' in request ? request.key : undefined,
    'prefix' in request ? request.prefix : undefined,
    'cursor' in request ? request.cursor : undefined,
  ];
  if (keys.some((key) => key !== undefined && !validStorageKey(key)))
    throw new Error('Invalid storage key');
  if (
    request.operation === 'set' &&
    new TextEncoder().encode(JSON.stringify(request.value)).length >
      maximumExtensionRequestBytes
  )
    throw new Error('Storage value is too large');
  if (
    new TextEncoder().encode(JSON.stringify(request)).length >
    maximumExtensionRequestBytes
  )
    throw new Error('Storage request is too large');
  return request;
};

export const frameDocument = `<!doctype html><meta charset="utf-8"><style>html,body{margin:0}</style><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-catalog-bootstrap' blob:; connect-src 'none'; img-src data:; style-src 'unsafe-inline'"><body><div id="root"></div><script nonce="catalog-bootstrap">
(() => { let port; let next = 0; const pending = new Map();
const call = (method, payload) => new Promise((resolve, reject) => { const id = String(++next); pending.set(id, {resolve,reject}); port.postMessage({type:'catalog:request.v1',id,method,payload}); }); const rejectPending = () => { for (const item of pending.values()) item.reject(new Error('Host request cancelled')); pending.clear(); };
window.addEventListener('message', async (event) => { if (event.source !== parent || event.data?.type !== 'catalog:init.v1' || !event.ports[0]) return; port = event.ports[0]; const capabilities = event.data.capabilities; port.onmessage = event => { const message = event.data; if (message?.type === 'catalog:shutdown.v1') { rejectPending(); Promise.resolve(cleanup?.()).finally(() => port.close()); return; } if (message?.type === 'catalog:context-update.v1') { if (!message.context || typeof message.context !== 'object' || Array.isArray(message.context)) return; globalThis.catalog.context = message.context; capabilities.includes('client.events') && root?.dispatchEvent(new CustomEvent('catalog:context-changed.v1', {detail: message.context})); return; } if (message?.type !== 'catalog:response.v1') return; const item = pending.get(message.id); if (!item) return; pending.delete(message.id); message.ok ? item.resolve(message.data) : item.reject(new Error(message.error || 'Host request failed')); }; let cleanup; let root; globalThis.catalog = { request: path => call('catalog.read', {path}), command: detail => call('command', detail), navigate: detail => call('navigate', detail), notify: detail => call('notify', detail), storage: { get: detail => call('storage.get', detail), set: detail => call('storage.set', detail), delete: detail => call('storage.delete', detail), list: detail => call('storage.list', detail) }, context: event.data.context, configuration: event.data.configuration }; try { const url = URL.createObjectURL(new Blob([event.data.artifact], {type:'text/javascript'})); const module = await import(url); URL.revokeObjectURL(url); if (typeof module.mount !== 'function') throw new Error('Extension must export mount(root, catalog)'); root = document.getElementById('root'); cleanup = await module.mount(root, globalThis.catalog); if (typeof cleanup !== 'function') cleanup = undefined; const resize = () => port.postMessage({type:'catalog:resize.v1', height: root.getBoundingClientRect().height}); new ResizeObserver(resize).observe(root); new MutationObserver(resize).observe(root, {childList:true, characterData:true, subtree:true}); if (event.data.capabilities.includes('client.events')) root.dispatchEvent(new CustomEvent('catalog:context-changed.v1', {detail:event.data.context})); port.postMessage({type:'catalog:ready.v1'}); resize(); } catch (error) { port.postMessage({type:'catalog:error.v1', error: String(error?.message || error)}); } }); })();
</script>`;

type Props = {
  contribution: ExtensionContribution;
  context?: Record<string, unknown>;
  onContentHeight?: (height: number) => void;
};

/** Executes one contribution in an opaque-origin document, never in Catalog's DOM. */
export const ExtensionFrame = ({
  contribution,
  context = {},
  onContentHeight,
}: Props) => {
  const iframe = useRef<HTMLIFrameElement>(null);
  const onContentHeightRef = useRef(onContentHeight);
  const portRef = useRef<MessagePort | undefined>(undefined);
  const contextRef = useRef(JSON.stringify(context));
  useEffect(() => {
    onContentHeightRef.current = onContentHeight;
  }, [onContentHeight]);
  const [error, setError] = useState<string>();
  const [ready, setReady] = useState(false);
  const [height, setHeight] = useState(defaultExtensionFrameHeight);
  const [loadedFrame, setLoadedFrame] = useState<string>();
  const navigate = useNavigate();
  const contextKey = JSON.stringify(context);
  useEffect(() => {
    contextRef.current = contextKey;
  }, [contextKey]);
  // Context changes are delivered through the broker. A contribution keeps its
  // frame and subscriptions until its release changes or it is unmounted.
  const frameKey = `${contribution.extension_id}:${contribution.id}:${contribution.release_id}`;

  useEffect(() => {
    if (loadedFrame !== frameKey) return;
    const frameContext = JSON.parse(contextRef.current) as Record<
      string,
      unknown
    >;
    let disposed = false;
    let port: MessagePort | undefined;
    const timer = window.setTimeout(
      () => !disposed && setError('The extension timed out while starting.'),
      extensionStartTimeout,
    );
    const start = async () => {
      try {
        const artifact = await getExtensionArtifact(
          contribution.extension_id,
          contribution.id,
        );
        if (disposed || !iframe.current?.contentWindow) return;
        const channel = new MessageChannel();
        port = channel.port1;
        portRef.current = port;
        port.onmessage = async ({ data }) => {
          if (!data || typeof data !== 'object') return;
          if (data.type === 'catalog:ready.v1') {
            window.clearTimeout(timer);
            return setReady(true);
          }
          if (data.type === 'catalog:error.v1')
            return setError('The extension could not be started.');
          if (
            data.type === 'catalog:resize.v1' &&
            typeof data.height === 'number' &&
            Number.isFinite(data.height)
          ) {
            const height = Math.min(
              Math.max(0, data.height),
              maximumExtensionFrameHeight,
            );
            setHeight(height);
            onContentHeightRef.current?.(height);
            return;
          }
          if (data.type !== 'catalog:request.v1' || typeof data.id !== 'string')
            return;
          const respond = (ok: boolean, value?: unknown) => {
            if (
              ok &&
              new TextEncoder().encode(JSON.stringify(value)).length >
                maximumExtensionResponseBytes
            )
              return respond(false);
            port?.postMessage({
              type: 'catalog:response.v1',
              id: data.id,
              ok,
              ...(ok ? { data: value } : { error: 'Request denied' }),
            });
          };
          try {
            if (
              data.method === 'navigate' &&
              contribution.capabilities.includes('client.navigation')
            ) {
              const detail = navigationSchema.parse(data.payload);
              await navigate({
                to: '/entities/$entityId',
                params: { entityId: detail.entity_id },
              });
              respond(true, null);
            } else if (
              data.method === 'notify' &&
              contribution.capabilities.includes('client.notification')
            ) {
              const notification = notificationSchema.parse(data.payload);
              toast.show(notification);
              respond(true, null);
            } else if (
              data.method.startsWith('storage.') &&
              contribution.capabilities.includes('storage.extension')
            ) {
              const operation = data.method.slice('storage.'.length);
              if (!['get', 'set', 'delete', 'list'].includes(operation))
                throw new Error('Storage request denied');
              const request = validateStorageRequest({
                ...(data.payload as Record<string, unknown>),
                operation,
              });
              respond(
                true,
                await extensionStorage(
                  contribution.extension_id,
                  contribution.id,
                  contribution.release_id,
                  request,
                ),
              );
            } else if (
              data.method === 'command' &&
              contribution.kind !== 'panel' &&
              contribution.capabilities.includes('client.commands')
            ) {
              const command = extensionCommandRequestSchema.parse({
                ...(data.payload as Record<string, unknown>),
                release_id: contribution.release_id,
              });
              if (
                new TextEncoder().encode(JSON.stringify(command.payload))
                  .length > maximumExtensionRequestBytes
              )
                throw new Error('Command payload is too large');
              respond(
                true,
                await extensionCommand(
                  contribution.extension_id,
                  contribution.id,
                  command,
                ),
              );
            } else if (
              data.method === 'catalog.read' &&
              contribution.capabilities.includes('catalog.read')
            ) {
              const { path } = catalogReadSchema.parse(data.payload);
              const revision = path.match(
                /^\/api\/blueprints\/([0-9a-f-]{36})\/versions\/([1-9][0-9]*)$/,
              );
              if (
                revision &&
                (frameContext.blueprint_id !== revision[1] ||
                  String(frameContext.blueprint_version) !== revision[2])
              )
                throw new Error(
                  'Blueprint revision is outside this outlet context',
                );
              const response = await apiFetch(path);
              if (!response.ok) throw new Error('Catalog request failed');
              const text = await response.text();
              if (
                new TextEncoder().encode(text).length >
                maximumExtensionResponseBytes
              )
                throw new Error('Catalog response is too large');
              respond(true, JSON.parse(text));
            } else {
              respond(false);
            }
          } catch {
            respond(false);
          }
        };
        iframe.current.contentWindow.postMessage(
          {
            type: 'catalog:init.v1',
            artifact,
            configuration: contribution.configuration,
            capabilities: contribution.capabilities,
            context: frameContext,
          },
          '*',
          [channel.port2],
        );
      } catch {
        if (!disposed) setError('The extension could not be loaded.');
      }
    };
    void start();
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      port?.postMessage({ type: 'catalog:shutdown.v1' });
      port?.close();
      if (portRef.current === port) portRef.current = undefined;
    };
  }, [contribution, frameKey, loadedFrame, navigate]);

  useEffect(() => {
    if (!ready) return;
    // Only the parent-owned port can update context; the opaque frame cannot
    // forge this versioned message or choose another contribution context.
    portRef.current?.postMessage({
      type: 'catalog:context-update.v1',
      context: JSON.parse(contextKey) as Record<string, unknown>,
    });
  }, [contextKey, ready]);

  if (error) return <Alert severity="warning">{error}</Alert>;
  return (
    <Box
      sx={{
        minHeight: ready ? 0 : defaultExtensionFrameHeight,
        position: 'relative',
      }}
    >
      <iframe
        aria-label={contribution.title ?? contribution.id}
        key={frameKey}
        onLoad={() => {
          setError(undefined);
          setHeight(defaultExtensionFrameHeight);
          setReady(false);
          setLoadedFrame(frameKey);
        }}
        ref={iframe}
        sandbox="allow-scripts"
        srcDoc={frameDocument}
        style={{ border: 0, height, opacity: ready ? 1 : 0, width: '100%' }}
        title={contribution.title ?? contribution.id}
      />
      {!ready && (
        <Box
          aria-live="polite"
          role="status"
          sx={{ inset: 0, p: 1, position: 'absolute' }}
        >
          <Skeleton animation="wave" height={20} variant="text" width="45%" />
          <Skeleton animation="wave" height={16} variant="text" width="75%" />
          <Typography sx={{ clip: 'rect(0 0 0 0)', position: 'absolute' }}>
            Loading extension content…
          </Typography>
        </Box>
      )}
    </Box>
  );
};

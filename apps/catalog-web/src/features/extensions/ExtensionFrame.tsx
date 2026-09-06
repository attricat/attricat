import { Alert, Box, CircularProgress } from '@mui/material';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { z } from 'zod';
import { apiFetch } from '../auth/request';
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
  new TextEncoder().encode(key).length <= 256 &&
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
    new TextEncoder().encode(JSON.stringify(request.value)).length > 65_536
  )
    throw new Error('Storage value is too large');
  if (new TextEncoder().encode(JSON.stringify(request)).length > 65_536)
    throw new Error('Storage request is too large');
  return request;
};

export const frameDocument = `<!doctype html><meta charset="utf-8"><style>html,body{margin:0}</style><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src 'nonce-catalog-bootstrap' blob:; connect-src 'none'; img-src data:; style-src 'unsafe-inline'"><body><div id="root"></div><script nonce="catalog-bootstrap">
(() => { let port; let next = 0; const pending = new Map();
const call = (method, payload) => new Promise((resolve, reject) => { const id = String(++next); pending.set(id, {resolve,reject}); port.postMessage({type:'catalog:request.v1',id,method,payload}); });
window.addEventListener('message', async (event) => { if (event.source !== parent || event.data?.type !== 'catalog:init.v1' || !event.ports[0]) return; port = event.ports[0]; port.onmessage = event => { const message = event.data; if (message?.type !== 'catalog:response.v1') return; const item = pending.get(message.id); if (!item) return; pending.delete(message.id); message.ok ? item.resolve(message.data) : item.reject(new Error(message.error || 'Host request failed')); }; globalThis.catalog = { request: path => call('catalog.read', {path}), command: detail => call('command', detail), navigate: detail => call('navigate', detail), notify: detail => call('notify', detail), storage: { get: detail => call('storage.get', detail), set: detail => call('storage.set', detail), delete: detail => call('storage.delete', detail), list: detail => call('storage.list', detail) }, context: event.data.context }; try { const url = URL.createObjectURL(new Blob([event.data.artifact], {type:'text/javascript'})); await import(url); URL.revokeObjectURL(url); const element = document.createElement(event.data.element); element.configuration = event.data.configuration; element.catalogContext = event.data.context; const root = document.getElementById('root'); root.append(element); const resize = () => port.postMessage({type:'catalog:resize.v1', height: root.getBoundingClientRect().height}); new ResizeObserver(resize).observe(root); if (event.data.capabilities.includes('client.events')) element.dispatchEvent(new CustomEvent('catalog:context-changed.v1', {detail:event.data.context})); port.postMessage({type:'catalog:ready.v1'}); resize(); } catch (error) { port.postMessage({type:'catalog:error.v1', error: String(error?.message || error)}); } }); })();
</script>`;

type Props = {
  contribution: ExtensionContribution;
  context?: Record<string, unknown>;
};

/** Executes one contribution in an opaque-origin document, never in Catalog's DOM. */
export const ExtensionFrame = ({ contribution, context = {} }: Props) => {
  const iframe = useRef<HTMLIFrameElement>(null);
  const [error, setError] = useState<string>();
  const [ready, setReady] = useState(false);
  const [height, setHeight] = useState(48);
  const [loadedFrame, setLoadedFrame] = useState<string>();
  const navigate = useNavigate();
  const contextKey = JSON.stringify(context);
  // A frame owns the custom element it registers; recreating it on a context
  // change prevents a second element from being appended to the same document.
  const frameKey = `${contribution.extension_id}:${contribution.id}:${contribution.release_id}:${contextKey}`;

  useEffect(() => {
    if (loadedFrame !== frameKey) return;
    const frameContext = JSON.parse(contextKey) as Record<string, unknown>;
    let disposed = false;
    let port: MessagePort | undefined;
    const timer = window.setTimeout(
      () => !disposed && setError('The extension timed out while starting.'),
      10_000,
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
          )
            return setHeight(Math.min(Math.max(0, data.height), 2048));
          if (data.type !== 'catalog:request.v1' || typeof data.id !== 'string')
            return;
          const respond = (ok: boolean, value?: unknown) => {
            if (
              ok &&
              new TextEncoder().encode(JSON.stringify(value)).length > 1_048_576
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
              // The frame cannot render host UI. A bounded event lets the host page opt in.
              window.dispatchEvent(
                new CustomEvent('catalog:extension-notify.v1', {
                  detail: notificationSchema.parse(data.payload),
                }),
              );
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
              contribution.capabilities.includes('client.commands')
            ) {
              const command = extensionCommandRequestSchema.parse({
                ...(data.payload as Record<string, unknown>),
                release_id: contribution.release_id,
              });
              if (
                new TextEncoder().encode(JSON.stringify(command.payload))
                  .length > 65_536
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
              if (new TextEncoder().encode(text).length > 1_048_576)
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
            element: contribution.element,
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
      port?.close();
    };
  }, [contribution, contextKey, frameKey, loadedFrame, navigate]);

  if (error) return <Alert severity="warning">{error}</Alert>;
  return (
    <Box sx={{ minHeight: ready ? 0 : 48 }}>
      <iframe
        aria-label={contribution.title ?? contribution.id}
        key={frameKey}
        onLoad={() => {
          setError(undefined);
          setHeight(48);
          setReady(false);
          setLoadedFrame(frameKey);
        }}
        ref={iframe}
        sandbox="allow-scripts"
        srcDoc={frameDocument}
        style={{ border: 0, height, width: '100%' }}
        title={contribution.title ?? contribution.id}
      />
      {!ready && <CircularProgress size={20} />}
    </Box>
  );
};

import { Alert, Box, CircularProgress } from '@mui/material';
import { useNavigate } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { z } from 'zod';
import { apiFetch } from '../auth/request';
import { getExtensionArtifact, type ExtensionContribution } from './api';

const notificationSchema = z
  .object({
    message: z.string().trim().min(1).max(512),
    severity: z.enum(['success', 'info', 'warning', 'error']).optional(),
  })
  .strict();
const navigationSchema = z.object({ entity_id: z.uuid() }).strict();
const catalogReadSchema = z
  .object({
    path: z.string().regex(/^\/api\/(?:entities|v1\/entities\/[0-9a-f-]{36})$/),
  })
  .strict();

const frameDocument = `<!doctype html><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="default-src 'none'; script-src blob:; connect-src 'none'; img-src data:; style-src 'unsafe-inline'"><body><div id="root"></div><script>
(() => { let port; let next = 0; const pending = new Map();
const call = (method, payload) => new Promise((resolve, reject) => { const id = String(++next); pending.set(id, {resolve,reject}); port.postMessage({type:'catalog:request.v1',id,method,payload}); });
window.addEventListener('message', async (event) => { if (event.source !== parent || event.data?.type !== 'catalog:init.v1' || !event.ports[0]) return; port = event.ports[0]; port.onmessage = event => { const message = event.data; if (message?.type !== 'catalog:response.v1') return; const item = pending.get(message.id); if (!item) return; pending.delete(message.id); message.ok ? item.resolve(message.data) : item.reject(new Error(message.error || 'Host request failed')); }; globalThis.catalog = { request: path => call('catalog.read', {path}), navigate: detail => call('navigate', detail), notify: detail => call('notify', detail), context: event.data.context }; try { const url = URL.createObjectURL(new Blob([event.data.artifact], {type:'text/javascript'})); await import(url); URL.revokeObjectURL(url); const element = document.createElement(event.data.element); element.configuration = event.data.configuration; element.catalogContext = event.data.context; document.getElementById('root').append(element); if (event.data.capabilities.includes('client.events')) element.dispatchEvent(new CustomEvent('catalog:context-changed.v1', {detail:event.data.context})); port.postMessage({type:'catalog:ready.v1'}); } catch (error) { port.postMessage({type:'catalog:error.v1', error: String(error?.message || error)}); } }); })();
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
  const navigate = useNavigate();

  useEffect(() => {
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
          if (data.type !== 'catalog:request.v1' || typeof data.id !== 'string')
            return;
          const respond = (ok: boolean, value?: unknown) =>
            port?.postMessage({
              type: 'catalog:response.v1',
              id: data.id,
              ok,
              ...(ok ? { data: value } : { error: 'Request denied' }),
            });
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
              data.method === 'catalog.read' &&
              contribution.capabilities.includes('catalog.read')
            ) {
              const { path } = catalogReadSchema.parse(data.payload);
              const response = await apiFetch(path);
              if (!response.ok) throw new Error('Catalog request failed');
              const text = await response.text();
              if (text.length > 1_048_576)
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
            context,
          },
          '*',
          [channel.port2],
        );
      } catch {
        if (!disposed) setError('The extension could not be loaded.');
      }
    };
    const onLoad = () => {
      setError(undefined);
      setReady(false);
      void start();
    };
    const node = iframe.current;
    node?.addEventListener('load', onLoad, { once: true });
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      port?.close();
      node?.replaceChildren();
    };
  }, [contribution, context, navigate]);

  if (error) return <Alert severity="warning">{error}</Alert>;
  return (
    <Box sx={{ minHeight: ready ? 0 : 48 }}>
      <iframe
        aria-label={contribution.title ?? contribution.id}
        ref={iframe}
        sandbox="allow-scripts"
        srcDoc={frameDocument}
        style={{ border: 0, width: '100%', minHeight: 48 }}
        title={contribution.title ?? contribution.id}
      />
      {!ready && <CircularProgress size={20} />}
    </Box>
  );
};

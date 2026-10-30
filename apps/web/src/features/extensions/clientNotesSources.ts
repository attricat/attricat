import type { QueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import { requestText } from '../../api/request';
import type { ExtensionContribution } from './api';
import { extensionPath } from './apiPaths';
import {
  contributionKinds,
  defaultFieldNoteTone,
  extensionCapabilities,
  extensionMessageTypes,
  fieldNoteLimits,
  fieldNoteTones,
  hostIcons,
} from './constants';
import { handleBrokerRequest, postBrokerResponse } from './extensionBroker';
import type {
  ClientNotesSource,
  FieldNotesResponse,
  RecordNotes,
} from './fieldNotesData';
import { handlerFrameDocument } from './frameDocument';

/** How long the host waits for a client source's notes, like a server one's. */
export const clientNotesTimeout = 2_000;

// The bounds of the shared field-notes contract; a client answer is checked
// against them here because no server sees it.
const badgeSchema = z
  .object({
    label: z.string().min(1).max(fieldNoteLimits.badgeLabel),
    tone: z.enum(fieldNoteTones).default(defaultFieldNoteTone),
    icon: z.enum(hostIcons).optional(),
    tooltip: z.string().min(1).max(fieldNoteLimits.badgeTooltip).optional(),
    action: z.string().min(1).max(fieldNoteLimits.badgeAction).optional(),
  })
  .strict();
const clientAnswerSchema = z
  .object({
    records: z.record(
      z.string(),
      z
        .object({
          attributes: z
            .record(
              z.string(),
              z
                .object({
                  badge: badgeSchema.optional(),
                  note: z.string().min(1).max(fieldNoteLimits.note).optional(),
                })
                .strict(),
            )
            .default({}),
          files: z
            .record(
              z.string(),
              z
                .object({ note: z.string().min(1).max(fieldNoteLimits.note) })
                .strict(),
            )
            .default({}),
          actions: z
            .record(
              z.string(),
              z.union([
                z
                  .object({
                    disabled: z
                      .string()
                      .min(1)
                      .max(fieldNoteLimits.disabledReason),
                  })
                  .strict(),
                z.object({ hidden: z.literal(true) }).strict(),
              ]),
            )
            .default({}),
        })
        .strict(),
    ),
    cacheable: z.boolean().optional(),
  })
  .strict();

type Handler = {
  ready: Promise<MessagePort>;
  pending: Map<string, (answer: { ok: boolean; result?: unknown }) => void>;
};

/** One headless frame per source release, started on first use. */
const handlers = new Map<string, Handler>();
let nextRequest = 0;

const handlerFor = (source: ClientNotesSource, queryClient: QueryClient) => {
  const key = `${source.extension_id}:${source.release_id}`;
  const existing = handlers.get(key);
  if (existing) return existing;
  const pending: Handler['pending'] = new Map();
  // The frame may only read; every other broker method is denied.
  const contribution = {
    extension_id: source.extension_id,
    extension_name: source.extension_name,
    id: 'record_field_notes',
    release_id: source.release_id,
    capabilities: source.capabilities.filter(
      (capability) => capability === extensionCapabilities.attricatRead,
    ),
    kind: contributionKinds.embedded,
    outlet: null,
  } as unknown as ExtensionContribution;
  const ready = new Promise<MessagePort>((resolve, reject) => {
    const frame = document.createElement('iframe');
    frame.sandbox.add('allow-scripts');
    frame.srcdoc = handlerFrameDocument;
    frame.hidden = true;
    frame.tabIndex = -1;
    frame.setAttribute('aria-hidden', 'true');
    frame.title = source.extension_name;
    frame.addEventListener(
      'load',
      () =>
        void (async () => {
          try {
            const artifact = await requestText(
              `${extensionPath(source.extension_id)}/field-notes-source`,
            );
            const channel = new MessageChannel();
            const port = channel.port1;
            port.onmessage = async ({ data }) => {
              if (data?.type === extensionMessageTypes.ready) resolve(port);
              else if (data?.type === extensionMessageTypes.error)
                reject(new Error(String(data.error)));
              else if (data?.type === extensionMessageTypes.handled) {
                pending.get(data.id)?.(data);
                pending.delete(data.id);
              } else if (
                data?.type === extensionMessageTypes.request &&
                typeof data.id === 'string'
              ) {
                try {
                  const result = await handleBrokerRequest(
                    { method: data.method, payload: data.payload },
                    {
                      contribution,
                      initialContext: {},
                      currentContext: () => ({}),
                      navigateToRecord: async () => {},
                      queryClient,
                      closeActionDialog: () => {},
                      onRunStarted: () => {},
                      downloadArtifact: () => {},
                    },
                  );
                  postBrokerResponse(port, data.id, { ok: true, data: result });
                } catch {
                  postBrokerResponse(port, data.id, { ok: false });
                }
              }
            };
            frame.contentWindow?.postMessage(
              {
                type: extensionMessageTypes.init,
                artifact,
                capabilities: contribution.capabilities,
                context: {},
              },
              '*',
              [channel.port2],
            );
          } catch (error) {
            reject(error);
          }
        })(),
      { once: true },
    );
    document.body.append(frame);
  });
  const handler = { ready, pending };
  handlers.set(key, handler);
  // A source that fails to start is retried on a later request.
  ready.catch(() => handlers.delete(key));
  return handler;
};

/**
 * Asks one client source for notes. Resolves to its raw answer, `'late'`
 * when it did not answer in time (it keeps starting), or `null` when it
 * failed.
 */
const askClientSource = async (
  source: ClientNotesSource,
  request: {
    record_ids: readonly string[];
    context_id: string | null;
    locale: string;
  },
  queryClient: QueryClient,
): Promise<unknown> => {
  const handler = handlerFor(source, queryClient);
  const answer = new Promise<unknown>((resolve) => {
    void handler.ready.then(
      (port) => {
        const id = String(++nextRequest);
        handler.pending.set(id, (reply) =>
          resolve(reply.ok ? reply.result : null),
        );
        port.postMessage({ type: extensionMessageTypes.handle, id, request });
      },
      () => resolve(null),
    );
  });
  const result = await Promise.race([
    answer,
    new Promise<'late'>((resolve) =>
      setTimeout(() => resolve('late'), clientNotesTimeout),
    ),
  ]);
  return result;
};

/**
 * Adds a client source's validated answer to `records`, only for records the
 * server confirmed the viewer can read, in extension ID order.
 */
export const mergeClientAnswer = (
  records: FieldNotesResponse['records'],
  source: ClientNotesSource,
  answer: unknown,
) => {
  const parsed = clientAnswerSchema.safeParse(answer);
  if (!parsed.success) return false;
  for (const [recordId, notes] of Object.entries(parsed.data.records)) {
    if (!records[recordId]) continue;
    records[recordId] = [
      ...records[recordId],
      {
        extension_id: source.extension_id,
        extension_name: source.extension_name,
        ...(notes as RecordNotes),
      },
    ].sort((left, right) =>
      left.extension_id.localeCompare(right.extension_id),
    );
  }
  return true;
};

/**
 * Adds every client source's notes to a server response. A source that is
 * still starting marks the response incomplete, so the caller asks again.
 */
export const withClientNotes = async (
  response: FieldNotesResponse,
  request: {
    record_ids: readonly string[];
    context_id: string | null;
    locale: string;
  },
  queryClient: QueryClient,
): Promise<FieldNotesResponse> => {
  if (!response.client_sources.length) return response;
  const records = { ...response.records };
  let incomplete = response.incomplete;
  await Promise.all(
    response.client_sources.map(async (source) => {
      const answer = await askClientSource(source, request, queryClient);
      if (answer === 'late') {
        incomplete = true;
        return;
      }
      if (answer) mergeClientAnswer(records, source, answer);
    }),
  );
  return { ...response, records, incomplete };
};

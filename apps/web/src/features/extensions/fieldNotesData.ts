import {
  useQuery,
  useQueryClient,
  type QueryClient,
} from '@tanstack/react-query';
import { useTranslation } from 'react-i18next';
import { z } from 'zod';
import { request } from '../../api/request';
import { withClientNotes } from './clientNotesSources';
import { extensionQueryKeys } from './queryKeys';
import {
  fieldNoteTones,
  fieldNotesRetryInterval,
  hostIcons,
  maximumFieldNotesAttempts,
} from './constants';

const badgeSchema = z.object({
  label: z.string().min(1),
  tone: z.enum(fieldNoteTones),
  icon: z.enum(hostIcons).optional(),
  tooltip: z.string().optional(),
  action: z.string().optional(),
});
export type FieldNoteBadge = z.infer<typeof badgeSchema>;

/** An action shown disabled with a reason, or not shown, for one record. */
const actionStateSchema = z.union([
  z.object({ disabled: z.string().min(1) }).strict(),
  z.object({ hidden: z.literal(true) }).strict(),
]);
export type ActionState = z.infer<typeof actionStateSchema>;

/** What one extension said about one record, after host validation. */
export const recordNotesSchema = z.object({
  attributes: z
    .record(
      z.string(),
      z.object({ badge: badgeSchema.optional(), note: z.string().optional() }),
    )
    .default({}),
  files: z.record(z.string(), z.object({ note: z.string() })).default({}),
  actions: z.record(z.string(), actionStateSchema).default({}),
});
export type RecordNotes = z.infer<typeof recordNotesSchema>;

const extensionNotesSchema = recordNotesSchema.extend({
  extension_id: z.string().min(1),
  extension_name: z.string().min(1),
});
export type ExtensionNotes = z.infer<typeof extensionNotesSchema>;

const fieldNotesSchema = z.object({
  records: z.record(z.string(), z.array(extensionNotesSchema)),
  incomplete: z.boolean(),
  client_sources: z.array(
    z.object({
      extension_id: z.string().min(1),
      extension_name: z.string().min(1),
      release_id: z.uuid(),
      capabilities: z.array(z.string()),
    }),
  ),
});
export type FieldNotesResponse = z.infer<typeof fieldNotesSchema>;
export type ClientNotesSource = FieldNotesResponse['client_sources'][number];

/**
 * Every provider's notes for `recordIds`: the server's providers through the
 * host API, then client-side providers in their sandboxed frames.
 */
export const fetchFieldNotes = async (
  recordIds: readonly string[],
  contextId: string | null,
  locale: string,
  queryClient: QueryClient,
) => {
  const body = { record_ids: recordIds, context_id: contextId, locale };
  const response = await request(
    '/api/records/extension-field-notes',
    fieldNotesSchema,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    },
  );
  return withClientNotes(response, body, queryClient);
};

/**
 * Every provider's notes for `recordIds` in one context and the viewer's
 * language. While a provider is still starting the query asks again.
 */
export const useFieldNotesQuery = ({
  contextId,
  queryKey,
  recordIds,
}: {
  contextId: string | null;
  /** Placed so that whatever refreshes these records refreshes their notes. */
  queryKey: readonly unknown[];
  recordIds: readonly string[];
}) => {
  const { i18n } = useTranslation();
  const queryClient = useQueryClient();
  return useQuery({
    queryKey: [...queryKey, i18n.language],
    queryFn: () =>
      fetchFieldNotes(recordIds, contextId, i18n.language, queryClient),
    enabled: recordIds.length > 0,
    refetchInterval: (query) =>
      query.state.data?.incomplete &&
      query.state.dataUpdateCount < maximumFieldNotesAttempts
        ? fieldNotesRetryInterval
        : false,
  });
};

/** The key a contribution's state is listed under: `extension:action`. */
export const actionStateKey = (extensionId: string, actionId: string) =>
  `${extensionId}:${actionId}`;

/** Each action's state for one record, by {@link actionStateKey}. */
export const actionStates = (
  data: FieldNotesResponse | undefined,
  recordId: string,
): Map<string, ActionState> =>
  new Map(
    (data?.records[recordId] ?? []).flatMap((extension) =>
      Object.entries(extension.actions).map(([actionId, state]) => [
        actionStateKey(extension.extension_id, actionId),
        state,
      ]),
    ),
  );

export type AttributeFieldNote = {
  extensionId: string;
  extensionName: string;
  badge?: FieldNoteBadge;
  note?: string;
};

/**
 * One record's notes, shared by its fields and its action surfaces; nothing
 * is asked without a record.
 */
export const useRecordFieldNotes = (
  recordId: string | undefined,
  contextId: string | null,
) =>
  useFieldNotesQuery({
    contextId,
    queryKey: extensionQueryKeys.fieldNotes(recordId ?? '', contextId),
    recordIds: recordId ? [recordId] : [],
  });

/** Lookups over every extension's notes for one record. */
export const fieldNoteLookups = (extensions: readonly ExtensionNotes[]) => ({
  attribute: (code: string): AttributeFieldNote[] =>
    extensions.flatMap((extension) => {
      const note = extension.attributes[code];
      return note
        ? [
            {
              extensionId: extension.extension_id,
              extensionName: extension.extension_name,
              ...note,
            },
          ]
        : [];
    }),
  file: (fileId: string): { extensionId: string; note: string }[] =>
    extensions.flatMap((extension) => {
      const note = extension.files[fileId];
      return note
        ? [{ extensionId: extension.extension_id, note: note.note }]
        : [];
    }),
});

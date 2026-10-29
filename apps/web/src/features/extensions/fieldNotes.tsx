import { Chip, Fade, Stack, Tooltip, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { createContext, useContext, useMemo, type ReactNode } from 'react';
import { z } from 'zod';
import { uuidSegment } from '../../api/pathSegments';
import { request } from '../../api/request';
import type { ActionTarget } from './actions';
import type { ExtensionContribution, ExtensionRuntimeScope } from './api';
import {
  contributionKinds,
  extensionOutlets,
  fieldNoteTones,
  fieldNotesRetryInterval,
  maximumFieldNotesAttempts,
  type FieldNoteTone,
} from './constants';
import { extensionQueryKeys } from './queryKeys';
import { useExtensionRuntime } from './useExtensionRuntime';
import { useInvokeExtensionAction } from './useInvokeExtensionAction';

const badgeSchema = z.object({
  label: z.string().min(1),
  tone: z.enum(fieldNoteTones),
  tooltip: z.string().optional(),
  action: z.string().optional(),
});
type FieldNoteBadge = z.infer<typeof badgeSchema>;

/** The host-validated notes every provider gave for one record. */
const fieldNotesSchema = z.object({
  extensions: z.array(
    z.object({
      extension_id: z.string().min(1),
      extension_name: z.string().min(1),
      attributes: z.record(
        z.string(),
        z.object({
          badge: badgeSchema.optional(),
          note: z.string().optional(),
        }),
      ),
      files: z.record(z.string(), z.object({ note: z.string() })),
    }),
  ),
  incomplete: z.boolean(),
});

const getRecordFieldNotes = (recordId: string, contextId: string | null) =>
  request(
    `/api/records/${uuidSegment(recordId)}/extension-field-notes${
      contextId ? `?context_id=${uuidSegment(contextId)}` : ''
    }`,
    fieldNotesSchema,
  );

type AttributeFieldNote = {
  extensionId: string;
  extensionName: string;
  badge?: FieldNoteBadge;
  note?: string;
};
type FileFieldNote = { extensionId: string; note: string };

type FieldNotes = {
  attribute: (code: string) => AttributeFieldNote[];
  file: (fileId: string) => FileFieldNote[];
  /** The badge-only action a badge names, when it is enabled. */
  badgeAction: (
    extensionId: string,
    actionId: string | undefined,
  ) => ExtensionContribution | undefined;
  target: ActionTarget;
};

const FieldNotesContext = createContext<FieldNotes | null>(null);

/**
 * Asks every extension once for the record's field notes and offers them to
 * the fields and files below. Without a provider, fields show no notes.
 */
export const FieldNotesProvider = ({
  children,
  contextId,
  recordId,
  runtimeScope,
}: {
  children: ReactNode;
  contextId: string | null;
  recordId: string;
  runtimeScope: ExtensionRuntimeScope;
}) => {
  const notes = useQuery({
    queryKey: extensionQueryKeys.fieldNotes(recordId, contextId),
    queryFn: () => getRecordFieldNotes(recordId, contextId),
    // A provider still starting answers on a later request.
    refetchInterval: (query) =>
      query.state.data?.incomplete &&
      query.state.dataUpdateCount < maximumFieldNotesAttempts
        ? fieldNotesRetryInterval
        : false,
  });
  const runtime = useExtensionRuntime(runtimeScope);
  const value = useMemo<FieldNotes>(() => {
    const extensions = notes.data?.extensions ?? [];
    return {
      attribute: (code) =>
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
      file: (fileId) =>
        extensions.flatMap((extension) => {
          const note = extension.files[fileId];
          return note
            ? [{ extensionId: extension.extension_id, note: note.note }]
            : [];
        }),
      badgeAction: (extensionId, actionId) =>
        actionId === undefined
          ? undefined
          : runtime.data?.find(
              (item) =>
                item.extension_id === extensionId &&
                item.id === actionId &&
                item.kind === contributionKinds.action &&
                item.outlet === extensionOutlets.recordAttributeAction,
            ),
      target: {
        blueprintId: runtimeScope.blueprintId,
        blueprintVersion: runtimeScope.blueprintVersion,
        contextId,
        recordIds: [recordId],
      },
    };
  }, [contextId, notes.data, recordId, runtime.data, runtimeScope]);
  return <FieldNotesContext value={value}>{children}</FieldNotesContext>;
};

const chipColor = (tone: FieldNoteTone) =>
  tone === 'neutral' ? 'default' : tone;

const NoteBadge = ({
  attributeCode,
  entry,
  notes,
}: {
  attributeCode: string;
  entry: AttributeFieldNote & { badge: FieldNoteBadge };
  notes: FieldNotes;
}) => {
  const invoke = useInvokeExtensionAction();
  const action = notes.badgeAction(entry.extensionId, entry.badge.action);
  // The tooltip says which extension the badge comes from.
  const title = entry.badge.tooltip
    ? `${entry.badge.tooltip} · ${entry.extensionName}`
    : entry.extensionName;
  return (
    <Tooltip describeChild title={title}>
      <Chip
        color={chipColor(entry.badge.tone)}
        label={entry.badge.label}
        onClick={
          action
            ? () =>
                void invoke(action, extensionOutlets.recordAttributeAction, {
                  ...notes.target,
                  attributeCode,
                })
            : undefined
        }
        size="small"
        variant="outlined"
      />
    </Tooltip>
  );
};

/**
 * The badges and notes extensions gave for one field, in one row below it.
 * Every field's notes arrive in the same response, so they appear together.
 */
export const FieldNotesRow = ({ attributeCode }: { attributeCode: string }) => {
  const notes = useContext(FieldNotesContext);
  const entries = notes?.attribute(attributeCode) ?? [];
  if (!notes || !entries.length) return null;
  return (
    <Fade appear in>
      <Stack
        direction="row"
        spacing={1}
        sx={{ alignItems: 'center', flexWrap: 'wrap' }}
        useFlexGap
      >
        {entries.map((entry) => [
          entry.badge && (
            <NoteBadge
              attributeCode={attributeCode}
              entry={{ ...entry, badge: entry.badge }}
              key={`${entry.extensionId}:badge`}
              notes={notes}
            />
          ),
          entry.note && (
            <Typography
              color="text.secondary"
              key={`${entry.extensionId}:note`}
              variant="caption"
            >
              {entry.note}
            </Typography>
          ),
        ])}
      </Stack>
    </Fade>
  );
};

/** Notes extensions gave for one file, below it. */
export const FileNotes = ({ fileId }: { fileId: string }) => {
  const entries = useContext(FieldNotesContext)?.file(fileId) ?? [];
  if (!entries.length) return null;
  return (
    <Fade appear in>
      <Stack spacing={0.5}>
        {entries.map((entry) => (
          <Typography
            color="text.secondary"
            key={entry.extensionId}
            variant="caption"
          >
            {entry.note}
          </Typography>
        ))}
      </Stack>
    </Fade>
  );
};

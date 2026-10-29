import { Chip, Fade, Stack, Tooltip, Typography } from '@mui/material';
import {
  createContext,
  useContext,
  useMemo,
  type MouseEvent,
  type ReactNode,
} from 'react';
import { compactIconSize } from '../../components/iconSizes';
import type { ActionTarget } from './actions';
import type { ExtensionContribution, ExtensionRuntimeScope } from './api';
import {
  contributionKinds,
  extensionOutlets,
  type FieldNoteTone,
} from './constants';
import {
  fieldNoteLookups,
  useRecordFieldNotes,
  type AttributeFieldNote,
  type FieldNoteBadge,
} from './fieldNotesData';
import { renderHostIcon } from './hostIcons';
import { useExtensionRuntime } from './useExtensionRuntime';
import { useInvokeExtensionAction } from './useInvokeExtensionAction';

type FieldNotes = {
  attribute: ReturnType<typeof fieldNoteLookups>['attribute'];
  file: ReturnType<typeof fieldNoteLookups>['file'];
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
  const notes = useRecordFieldNotes(recordId, contextId);
  const runtime = useExtensionRuntime(runtimeScope);
  const value = useMemo<FieldNotes>(
    () => ({
      ...fieldNoteLookups(notes.data?.records[recordId] ?? []),
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
    }),
    [contextId, notes.data, recordId, runtime.data, runtimeScope],
  );
  return <FieldNotesContext value={value}>{children}</FieldNotesContext>;
};

const chipColor = (tone: FieldNoteTone) =>
  tone === 'neutral' ? 'default' : tone;

/**
 * A badge as a small chip whose tooltip also names its extension; a button
 * when it runs one of the extension's actions.
 */
export const NoteBadge = ({
  badge,
  extensionName,
  onClick,
}: {
  badge: FieldNoteBadge;
  extensionName: string;
  onClick?: (event: MouseEvent) => void;
}) => {
  const title = badge.tooltip
    ? `${badge.tooltip} · ${extensionName}`
    : extensionName;
  return (
    <Tooltip describeChild title={title}>
      <Chip
        color={chipColor(badge.tone)}
        icon={
          badge.icon ? renderHostIcon(badge.icon, compactIconSize) : undefined
        }
        label={badge.label}
        onClick={onClick}
        size="small"
        variant="outlined"
      />
    </Tooltip>
  );
};

const FieldBadge = ({
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
  return (
    <NoteBadge
      badge={entry.badge}
      extensionName={entry.extensionName}
      onClick={
        action
          ? () =>
              void invoke(action, extensionOutlets.recordAttributeAction, {
                ...notes.target,
                attributeCode,
              })
          : undefined
      }
    />
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
            <FieldBadge
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

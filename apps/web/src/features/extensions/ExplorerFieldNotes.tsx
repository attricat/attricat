import { Stack } from '@mui/material';
import { useQueries, useQueryClient } from '@tanstack/react-query';
import { useContext, useMemo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { accessTargetKinds, capabilities } from '../auth/capabilities';
import { useAccess, useCan } from '../auth/useCan';
import { recordQueryKeys } from '../records/queryKeys';
import type { ExtensionRuntimeScope } from './api';
import type { ActionTarget } from './actions';
import {
  explorerFieldNotesChunkSize,
  extensionOutlets,
  fieldNotesRetryInterval,
  maximumFieldNotesAttempts,
} from './constants';
import { offeredBadgeAction } from './actionAvailability';
import { NoteBadge } from './fieldNotes';
import {
  actionStates,
  fetchFieldNotes,
  fieldNoteLookups,
  type FieldNotesResponse,
} from './fieldNotesData';
import {
  ExplorerFieldNotesContext,
  type ExplorerFieldNotes,
} from './explorerFieldNotesContext';
import { useExtensionRuntime } from './useExtensionRuntime';
import { useInvokeExtensionAction } from './useInvokeExtensionAction';

const chunks = (ids: readonly string[]) =>
  Array.from(
    { length: Math.ceil(ids.length / explorerFieldNotesChunkSize) },
    (_, index) =>
      ids.slice(
        index * explorerFieldNotesChunkSize,
        (index + 1) * explorerFieldNotesChunkSize,
      ),
  );

/**
 * Asks for the field notes of the Explorer's loaded rows, one request per
 * chunk of rows so loading more rows asks only about the new ones. The notes
 * live under the search keys, so edits that refresh results refresh them.
 */
export const ExplorerFieldNotesProvider = ({
  blueprintCode,
  children,
  contextId,
  recordIds,
  runtimeScope,
}: {
  blueprintCode: string;
  children: ReactNode;
  contextId: string | null;
  recordIds: readonly string[];
  runtimeScope: ExtensionRuntimeScope;
}) => {
  const { i18n } = useTranslation();
  const queryClient = useQueryClient();
  const { blueprintId, blueprintVersion } = runtimeScope;
  const records = useQueries({
    queries: chunks(recordIds).map((ids) => ({
      queryKey: [
        ...recordQueryKeys.searches(),
        'extension-field-notes',
        ids,
        contextId,
        i18n.language,
      ],
      queryFn: () =>
        fetchFieldNotes(ids, contextId, i18n.language, queryClient),
      refetchInterval: (query: {
        state: { data?: FieldNotesResponse; dataUpdateCount: number };
      }) =>
        query.state.data?.incomplete &&
        query.state.dataUpdateCount < maximumFieldNotesAttempts
          ? fieldNotesRetryInterval
          : false,
    })),
    // One record map for every chunk, stable while no chunk changes.
    combine: (results) =>
      Object.assign(
        {},
        ...results.map((result) => result.data?.records ?? {}),
      ) as FieldNotesResponse['records'],
  });
  // Badge actions that need write access are not offered to readers.
  const writeAccess = useAccess(
    capabilities.recordsWrite,
    accessTargetKinds.record,
    recordIds,
  );
  const writable = writeAccess.isPending
    ? undefined
    : recordIds.filter((id) => writeAccess.can(id)).join();
  const value = useMemo<ExplorerFieldNotes>(() => {
    const writableIds = writable?.split(',');
    const merged: FieldNotesResponse = {
      records: records,
      incomplete: false,
      client_sources: [],
    };
    return {
      notes: (recordId) => records[recordId] ?? [],
      states: (recordId) => actionStates(merged, recordId),
      canWrite: (recordId) => writableIds?.includes(recordId),
      runtimeScope: { blueprintId, blueprintVersion },
      blueprintCode,
      contextId,
    };
  }, [
    blueprintCode,
    blueprintId,
    blueprintVersion,
    contextId,
    records,
    writable,
  ]);
  return (
    <ExplorerFieldNotesContext value={value}>
      {children}
    </ExplorerFieldNotesContext>
  );
};

/**
 * The badges extensions gave for one cell, after its value. A cell has no
 * room for notes, so a badge's tooltip carries the field's note too.
 */
export const ExplorerCellNotes = ({
  attributeCode,
  children,
  recordId,
  recordTags,
}: {
  attributeCode: string;
  children: ReactNode;
  recordId: string;
  recordTags: readonly string[];
}) => {
  const notes = useContext(ExplorerFieldNotesContext);
  const runtime = useExtensionRuntime(notes?.runtimeScope);
  const invoke = useInvokeExtensionAction();
  const canRunCommands = useCan(capabilities.recordsWrite);
  const entries = notes
    ? fieldNoteLookups(notes.notes(recordId)).attribute(attributeCode)
    : [];
  const badges = entries.flatMap((entry) => (entry.badge ? [entry] : []));
  if (!notes || !badges.length) return children;
  const target: ActionTarget = {
    canRunCommands,
    blueprintId: notes.runtimeScope.blueprintId,
    blueprintVersion: notes.runtimeScope.blueprintVersion,
    blueprintCode: notes.blueprintCode,
    contextId: notes.contextId,
    recordIds: [recordId],
    recordTags: [recordTags],
    canWrite: notes.canWrite(recordId),
  };
  const states = notes.states(recordId);
  return (
    <Stack direction="row" spacing={0.5} sx={{ alignItems: 'center' }}>
      {children}
      {badges.map((entry) => {
        const badge = entry.badge!;
        const action = offeredBadgeAction(
          runtime.data,
          entry.extensionId,
          badge.action,
          target,
          states,
        );
        return (
          <NoteBadge
            badge={{
              ...badge,
              tooltip:
                [badge.tooltip, entry.note].filter(Boolean).join(' · ') ||
                undefined,
            }}
            extensionName={entry.extensionName}
            key={entry.extensionId}
            onClick={
              action
                ? (event) => {
                    // The row opens the record on click; the badge does not.
                    event.stopPropagation();
                    void invoke(
                      action,
                      extensionOutlets.recordAttributeAction,
                      { ...target, attributeCode },
                    );
                  }
                : undefined
            }
          />
        );
      })}
    </Stack>
  );
};

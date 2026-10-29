import { Stack } from '@mui/material';
import { useQueries, useQueryClient } from '@tanstack/react-query';
import { useContext, useMemo, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { recordQueryKeys } from '../records/queryKeys';
import type { ExtensionRuntimeScope } from './api';
import {
  contributionKinds,
  explorerFieldNotesChunkSize,
  extensionOutlets,
  fieldNotesRetryInterval,
  maximumFieldNotesAttempts,
} from './constants';
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
  children,
  contextId,
  recordIds,
  runtimeScope,
}: {
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
  const value = useMemo<ExplorerFieldNotes>(() => {
    const merged: FieldNotesResponse = {
      records: records,
      incomplete: false,
      client_sources: [],
    };
    return {
      notes: (recordId) => records[recordId] ?? [],
      states: (recordId) => actionStates(merged, recordId),
      runtimeScope: { blueprintId, blueprintVersion },
      contextId,
    };
  }, [blueprintId, blueprintVersion, contextId, records]);
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
}: {
  attributeCode: string;
  children: ReactNode;
  recordId: string;
}) => {
  const notes = useContext(ExplorerFieldNotesContext);
  const runtime = useExtensionRuntime(notes?.runtimeScope);
  const invoke = useInvokeExtensionAction();
  const entries = notes
    ? fieldNoteLookups(notes.notes(recordId)).attribute(attributeCode)
    : [];
  const badges = entries.flatMap((entry) => (entry.badge ? [entry] : []));
  if (!notes || !badges.length) return children;
  return (
    <Stack direction="row" spacing={0.5} sx={{ alignItems: 'center' }}>
      {children}
      {badges.map((entry) => {
        const badge = entry.badge!;
        const action = badge.action
          ? runtime.data?.find(
              (item) =>
                item.extension_id === entry.extensionId &&
                item.id === badge.action &&
                item.kind === contributionKinds.action &&
                item.outlet === extensionOutlets.recordAttributeAction,
            )
          : undefined;
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
                      {
                        blueprintId: notes.runtimeScope.blueprintId,
                        blueprintVersion: notes.runtimeScope.blueprintVersion,
                        contextId: notes.contextId,
                        recordIds: [recordId],
                        attributeCode,
                      },
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

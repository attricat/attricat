import { useQueryClient } from '@tanstack/react-query';
import { useRef, useState } from 'react';
import { ApiRequestError } from '../../api/request';
import { updateRecord, type Attribute } from './api';
import { STALE_RECORD_ERROR_CODE } from './constants';
import type { FieldEditRules } from './recordForm';
import { fieldSaveRequest } from './recordFieldSaves';
import { invalidateRecord } from './invalidateRecord';

type Options = {
  recordId: string;
  contextId: string | null;
  /** Attributes the user may change in this context. */
  attributes: readonly Attribute[];
  /** The server's saved field values of this context. */
  savedFields: Readonly<Record<string, string>>;
  /** The server's version of the record that `savedFields` belong to. */
  updatedAt?: string;
  fieldRules?: ReadonlyMap<string, FieldEditRules>;
  /** Called when changes start or stop waiting to be saved. */
  onPendingChange?: (pending: boolean) => void;
};

export type ConflictResolution = 'keepMine' | 'useTheirs';

type State = {
  /** The version `saved` belongs to; saves are made against it. */
  version?: string;
  saved: Record<string, string>;
  pending: Record<string, string>;
  saving: boolean;
  error: Error | null;
  conflict: boolean;
};

const isStaleRecord = (error: unknown) =>
  error instanceof ApiRequestError && error.code === STALE_RECORD_ERROR_CODE;

const isNewer = (candidate?: string, baseline?: string) =>
  candidate !== undefined &&
  (baseline === undefined || Date.parse(candidate) > Date.parse(baseline));

/**
 * Saves committed field values one request at a time, each against the
 * version the previous save produced. A rejected change stays pending and is
 * resent with the next commit, so values that are only valid together (for
 * example several missing required fields) can be filled one by one.
 */
export const useRecordFieldSaves = ({
  recordId,
  contextId,
  attributes,
  savedFields,
  updatedAt,
  fieldRules,
  onPendingChange,
}: Options) => {
  const client = useQueryClient();
  const [state, setState] = useState<State>(() => ({
    version: updatedAt,
    saved: { ...savedFields },
    pending: {},
    saving: false,
    error: null,
    conflict: false,
  }));
  // The queue runs outside render: event handlers and the save loop read and
  // write these refs, and React state mirrors them for render.
  const current = useRef(state);
  const inFlight = useRef<Readonly<Record<string, string>> | null>(null);
  // What the latest commit knew about the editable attributes.
  const request = useRef({ attributes, fieldRules });

  const publish = (next: Partial<State>) => {
    const wasPending = Object.keys(current.current.pending).length > 0;
    current.current = { ...current.current, ...next };
    setState(current.current);
    const isPending = Object.keys(current.current.pending).length > 0;
    if (wasPending !== isPending) onPendingChange?.(isPending);
  };

  // While idle, a newer server version (another editor, an attachment, a file
  // upload) is the baseline instead of a conflict. Render shows it; the next
  // commit adopts it.
  const idle =
    !state.saving && !state.conflict && Object.keys(state.pending).length === 0;
  const serverIsNewer = idle && isNewer(updatedAt, state.version);
  const adoptServerVersion = () => {
    const { pending, conflict } = current.current;
    if (
      inFlight.current ||
      conflict ||
      Object.keys(pending).length > 0 ||
      !isNewer(updatedAt, current.current.version)
    )
      return;
    publish({ version: updatedAt, saved: { ...savedFields } });
  };

  const flush = async (): Promise<void> => {
    const { pending, saved, conflict, version } = current.current;
    if (inFlight.current || conflict || Object.keys(pending).length === 0)
      return;
    const batch = { ...pending };
    inFlight.current = batch;
    const sentVersion = version;
    publish({ saving: true });
    try {
      const record = await updateRecord(recordId, {
        ...(version ? { expected_updated_at: version } : {}),
        ...fieldSaveRequest(
          request.current.attributes,
          batch,
          saved,
          contextId,
          request.current.fieldRules,
        ),
      });
      // A value changed again while this save ran stays pending.
      const remaining = Object.fromEntries(
        Object.entries(current.current.pending).filter(
          ([code, value]) => batch[code] !== value,
        ),
      );
      inFlight.current = null;
      // A file change noted while this save ran may be newer than its result.
      const noted = current.current.version;
      publish({
        version: isNewer(noted, record.updated_at)
          ? noted
          : (record.updated_at ?? noted),
        saved: { ...current.current.saved, ...batch },
        pending: remaining,
        saving: false,
        error: null,
      });
      void invalidateRecord(client, recordId);
      return flush();
    } catch (error) {
      inFlight.current = null;
      // This editor's own file change moved the version while the save ran;
      // resend against it instead of reporting another editor's change.
      if (isStaleRecord(error) && current.current.version !== sentVersion) {
        publish({ saving: false });
        return flush();
      }
      publish({
        saving: false,
        error: error instanceof Error ? error : new Error(String(error)),
        conflict: isStaleRecord(error),
      });
    }
  };

  const commitMany = (values: Readonly<Record<string, string>>) => {
    request.current = { attributes, fieldRules };
    adoptServerVersion();
    const { saved, pending } = current.current;
    const next = { ...pending };
    for (const [code, value] of Object.entries(values)) {
      // Returning to the saved value cancels a change that is not yet sent.
      if (value === saved[code] && inFlight.current?.[code] === undefined)
        delete next[code];
      else next[code] = value;
    }
    publish({ pending: next });
    void flush();
  };

  return {
    /** Saved values overlaid with changes that are not saved yet. */
    fields: {
      ...(serverIsNewer ? savedFields : state.saved),
      ...state.pending,
    },
    pending: state.pending,
    saving: state.saving,
    error: state.error,
    conflict: state.conflict,
    commit: (code: string, value: string) => commitMany({ [code]: value }),
    commitMany,
    /** Drops an unsaved change and shows the saved value again. */
    revert: (code: string) => {
      const pending = { ...current.current.pending };
      delete pending[code];
      // With nothing left to save, the failure no longer applies.
      const settled =
        Object.keys(pending).length === 0 && !current.current.conflict;
      publish({ pending, ...(settled && { error: null }) });
    },
    /** Resends pending changes, for example after a network failure. */
    retry: () => {
      request.current = { attributes, fieldRules };
      publish({ error: null });
      void flush();
    },
    /** Moves the version forward after a change saved elsewhere (files). */
    noteRecordUpdated: (nextUpdatedAt: string) => {
      // Uploads finish in any order; an older result never moves it back.
      if (isNewer(nextUpdatedAt, current.current.version))
        publish({ version: nextUpdatedAt });
    },
    /**
     * Continues after another editor's change: rebases on the latest saved
     * values and either resends or discards this editor's pending changes.
     */
    resolveConflict: (
      resolution: ConflictResolution,
      latest: { savedFields: Record<string, string>; updatedAt?: string },
    ) => {
      request.current = { attributes, fieldRules };
      const pending =
        resolution === 'useTheirs'
          ? {}
          : Object.fromEntries(
              Object.entries(current.current.pending).filter(
                ([code, value]) => latest.savedFields[code] !== value,
              ),
            );
      publish({
        version: latest.updatedAt,
        saved: { ...latest.savedFields },
        pending,
        error: null,
        conflict: false,
      });
      void flush();
    },
  };
};

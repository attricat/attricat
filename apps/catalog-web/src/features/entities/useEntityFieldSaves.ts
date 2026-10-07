import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useRef, useState } from 'react';
import { ApiRequestError } from '../../api/request';
import { updateEntity, type Attribute, type Entity } from './api';
import { STALE_ENTITY_ERROR_CODE } from './constants';
import type { FieldEditRules } from './entityForm';
import { fieldSaveRequest } from './entityFieldSaves';
import { invalidateEntity } from './invalidateEntity';

type Options = {
  entityId: string;
  contextId: string | null;
  /** Attributes the user may change in this context. */
  attributes: readonly Attribute[];
  /** Saved field values of this context when the editor opened. */
  savedFields: Readonly<Record<string, string>>;
  updatedAt?: string;
  fieldRules?: ReadonlyMap<string, FieldEditRules>;
  onSaved?: (entity: Entity) => void;
};

export type ConflictResolution = 'keepMine' | 'useTheirs';

type State = {
  saved: Record<string, string>;
  pending: Record<string, string>;
  saving: boolean;
  error: Error | null;
  conflict: boolean;
};

const isStaleEntity = (error: unknown) =>
  error instanceof ApiRequestError && error.code === STALE_ENTITY_ERROR_CODE;

/**
 * Saves committed field values one request at a time, each against the
 * version the previous save produced. A rejected change stays pending and is
 * resent with the next commit, so values that are only valid together (for
 * example several missing required fields) can be filled one by one.
 */
export const useEntityFieldSaves = ({
  entityId,
  contextId,
  attributes,
  savedFields,
  updatedAt,
  fieldRules,
  onSaved,
}: Options) => {
  const client = useQueryClient();
  const [state, setState] = useState<State>(() => ({
    saved: { ...savedFields },
    pending: {},
    saving: false,
    error: null,
    conflict: false,
  }));
  // The queue reads and writes through refs; React state mirrors it for render.
  const current = useRef(state);
  const version = useRef(updatedAt);
  const inFlight = useRef<Readonly<Record<string, string>> | null>(null);
  const options = useRef({ attributes, fieldRules, onSaved });
  const active = useRef(true);
  useEffect(() => {
    options.current = { attributes, fieldRules, onSaved };
  });
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);

  const publish = (next: Partial<State>) => {
    current.current = { ...current.current, ...next };
    if (active.current) setState(current.current);
  };

  // While idle, a newer server version (another editor, an attachment, a
  // file upload) becomes the baseline instead of surfacing as a conflict.
  useEffect(() => {
    const idle =
      !inFlight.current &&
      !current.current.conflict &&
      Object.keys(current.current.pending).length === 0;
    if (!idle || !updatedAt || updatedAt === version.current) return;
    version.current = updatedAt;
    publish({ saved: { ...savedFields } });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- only a version change rebases
  }, [updatedAt]);

  const flush = async (): Promise<void> => {
    const { pending, saved, conflict } = current.current;
    if (inFlight.current || conflict || Object.keys(pending).length === 0)
      return;
    const batch = { ...pending };
    inFlight.current = batch;
    publish({ saving: true });
    try {
      const entity = await updateEntity(entityId, {
        ...(version.current ? { expected_updated_at: version.current } : {}),
        ...fieldSaveRequest(
          options.current.attributes,
          batch,
          saved,
          contextId,
          options.current.fieldRules,
        ),
      });
      version.current = entity.updated_at ?? version.current;
      // A value changed again while this save ran stays pending.
      const remaining = Object.fromEntries(
        Object.entries(current.current.pending).filter(
          ([code, value]) => batch[code] !== value,
        ),
      );
      inFlight.current = null;
      publish({
        saved: { ...current.current.saved, ...batch },
        pending: remaining,
        saving: false,
        error: null,
      });
      options.current.onSaved?.(entity);
      void invalidateEntity(client, entityId);
      return flush();
    } catch (error) {
      inFlight.current = null;
      publish({
        saving: false,
        error: error instanceof Error ? error : new Error(String(error)),
        conflict: isStaleEntity(error),
      });
    }
  };

  const commitMany = (values: Readonly<Record<string, string>>) => {
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
    fields: { ...state.saved, ...state.pending },
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
      publish({ pending });
    },
    /** Resends pending changes, for example after a network failure. */
    retry: () => {
      publish({ error: null });
      void flush();
    },
    /** Moves the version forward after a change saved elsewhere (files). */
    noteEntityUpdated: (nextUpdatedAt: string) => {
      version.current = nextUpdatedAt;
    },
    /**
     * Continues after another editor's change: rebases on the latest saved
     * values and either resends or discards this editor's pending changes.
     */
    resolveConflict: (
      resolution: ConflictResolution,
      latest: { savedFields: Record<string, string>; updatedAt?: string },
    ) => {
      version.current = latest.updatedAt;
      const pending =
        resolution === 'useTheirs'
          ? {}
          : Object.fromEntries(
              Object.entries(current.current.pending).filter(
                ([code, value]) => latest.savedFields[code] !== value,
              ),
            );
      publish({
        saved: { ...latest.savedFields },
        pending,
        error: null,
        conflict: false,
      });
      void flush();
    },
  };
};

import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { z } from 'zod';
import { currentSession } from '../auth/api';
import { authQueryKeys } from '../auth/queryKeys';
import { draftWriteDelayMs, type DraftEditor } from './constants';
import {
  draftStorageKey,
  readDraft,
  removeDraft,
  writeDraft,
  type StoredDraft,
} from './draftStorage';
import { useBeforeUnloadWarning } from './useBeforeUnloadWarning';

export type PendingDraft<T> = StoredDraft<T> & {
  /** Whether the loaded source differs from the one the draft was edited from. */
  sourceChanged: boolean;
};

type EditorDraftOptions<T> = {
  editor: DraftEditor;
  /** Resource identity and source version/context, in a stable order. */
  resource: readonly (string | number)[];
  /** True once the source form has loaded its current values. */
  ready: boolean;
  /** Whether `value` differs from the loaded source. */
  dirty: boolean;
  /** Current non-sensitive form values; never include secrets or files. */
  value: T;
  /** Fingerprint of the loaded source, or null when there is none. */
  source: string | null;
  /** Validates restored values; keep it referentially stable. */
  schema: z.ZodType<T>;
};

type PendingWrite<T> = { key: string; source: string | null; value: T };

/**
 * Keeps an editor's unsaved values in tab-scoped session storage. A draft
 * found for the loaded form is only offered through `pending`; it is applied
 * when the caller takes the value returned by `restore`.
 */
export const useEditorDraft = <T>({
  editor,
  resource,
  ready,
  dirty,
  value,
  source,
  schema,
}: EditorDraftOptions<T>) => {
  const session = useQuery({
    queryKey: authQueryKeys.session(),
    queryFn: currentSession,
  });
  const workspaceId = session.data?.workspace_id;
  const userId = session.data?.user_id;
  const resourceKey = JSON.stringify(resource);
  const key = useMemo(
    () =>
      ready && workspaceId && userId
        ? draftStorageKey({
            editor,
            resource: JSON.parse(resourceKey) as (string | number)[],
            userId,
            workspaceId,
          })
        : undefined,
    [editor, ready, resourceKey, userId, workspaceId],
  );
  // Read once per key so later writes from this editor are not re-offered.
  const stored = useMemo(
    () => (key ? readDraft(key, schema) : undefined),
    [key, schema],
  );
  const [resolvedKey, setResolvedKey] = useState<string>();
  const pending: PendingDraft<T> | undefined =
    key && stored && resolvedKey !== key
      ? { ...stored, sourceChanged: stored.source !== source }
      : undefined;
  const tracking = Boolean(key) && !pending;

  const pendingWrite = useRef<PendingWrite<T> | undefined>(undefined);
  const writeTimer = useRef<number | undefined>(undefined);
  const clearedKey = useRef<string | undefined>(undefined);
  const wasDirty = useRef(false);

  const cancelWrite = useCallback(() => {
    window.clearTimeout(writeTimer.current);
    writeTimer.current = undefined;
    pendingWrite.current = undefined;
  }, []);

  const flush = useCallback(() => {
    const write = pendingWrite.current;
    cancelWrite();
    if (write)
      writeDraft(write.key, {
        savedAt: new Date().toISOString(),
        source: write.source,
        value: write.value,
      });
  }, [cancelWrite]);

  useEffect(() => {
    if (!key || !tracking || clearedKey.current === key) return;
    if (!dirty) {
      cancelWrite();
      // Only edits reverted back to the source remove the draft; a restored
      // draft may be applied a render after it was accepted.
      if (wasDirty.current) removeDraft(key);
      wasDirty.current = false;
      return;
    }
    wasDirty.current = true;
    pendingWrite.current = { key, source, value };
    window.clearTimeout(writeTimer.current);
    writeTimer.current = window.setTimeout(flush, draftWriteDelayMs);
  }, [cancelWrite, dirty, flush, key, source, tracking, value]);

  useEffect(() => {
    const flushWhenHidden = () => {
      if (document.visibilityState === 'hidden') flush();
    };
    document.addEventListener('visibilitychange', flushWhenHidden);
    window.addEventListener('pagehide', flush);
    return () => {
      document.removeEventListener('visibilitychange', flushWhenHidden);
      window.removeEventListener('pagehide', flush);
      // Ordinary navigation keeps the draft for a later return.
      flush();
    };
  }, [flush]);

  // Session storage does not outlive the tab, so closing it still loses edits.
  useBeforeUnloadWarning(ready && dirty);

  /** Accepts the pending draft and returns the values to apply. */
  const restore = () => {
    if (!pending) return undefined;
    setResolvedKey(key);
    return pending.value;
  };

  /** Removes the pending draft without changing the loaded form. */
  const discard = () => {
    if (!key) return;
    removeDraft(key);
    setResolvedKey(key);
  };

  /** Removes the draft after a confirmed save and stops tracking this form. */
  const clear = useCallback(() => {
    cancelWrite();
    if (!key) return;
    clearedKey.current = key;
    removeDraft(key);
  }, [cancelWrite, key]);

  return { clear, discard, pending, restore };
};

import { z } from 'zod';
import {
  draftFormatVersion,
  draftStorageKeyPrefix,
  maxDraftLength,
  type DraftEditor,
} from './constants';

export type DraftScope = {
  workspaceId: string;
  userId: string;
  editor: DraftEditor;
  /** Resource identity and source version/context, in a stable order. */
  resource: readonly (string | number)[];
};

export type StoredDraft<T> = {
  savedAt: string;
  /** Fingerprint of the loaded source the draft was edited from. */
  source: string | null;
  value: T;
};

const draftEnvelopeSchema = z.object({
  format: z.literal(draftFormatVersion),
  savedAt: z.string().datetime(),
  source: z.string().nullable(),
  value: z.unknown(),
});

const draftKeySeparator = ':';

export const draftStorageKey = ({
  workspaceId,
  userId,
  editor,
  resource,
}: DraftScope) =>
  [
    `${draftStorageKeyPrefix}.v${draftFormatVersion}`,
    ...[workspaceId, userId, editor, ...resource].map((part) =>
      encodeURIComponent(String(part)),
    ),
  ].join(draftKeySeparator);

// Accessing `sessionStorage` itself throws when storage is disabled.
const storage = () => {
  try {
    return window.sessionStorage;
  } catch {
    return undefined;
  }
};

/**
 * Resources (by their first identity part) that have a stored draft for this
 * user and editor. Used to warn before an action reads saved data only.
 */
export const draftResourceIds = ({
  workspaceId,
  userId,
  editor,
}: Omit<DraftScope, 'resource'>) => {
  const target = storage();
  if (!target) return new Set<string>();
  const prefix = draftStorageKey({ workspaceId, userId, editor, resource: [] });
  const ids = new Set<string>();
  try {
    for (let index = 0; index < target.length; index += 1) {
      const key = target.key(index);
      if (!key?.startsWith(`${prefix}${draftKeySeparator}`)) continue;
      const [resourceId] = key
        .slice(prefix.length + draftKeySeparator.length)
        .split(draftKeySeparator);
      if (resourceId) ids.add(decodeURIComponent(resourceId));
    }
  } catch {
    // Storage failures must never block an action.
  }
  return ids;
};

export const removeDraft = (key: string) => {
  try {
    storage()?.removeItem(key);
  } catch {
    // Storage failures must never interrupt editing.
  }
};

export const readDraft = <T>(
  key: string,
  valueSchema: z.ZodType<T>,
): StoredDraft<T> | undefined => {
  let serialized: string | null | undefined;
  try {
    serialized = storage()?.getItem(key);
  } catch {
    return undefined;
  }
  if (!serialized) return undefined;
  if (serialized.length > maxDraftLength) {
    removeDraft(key);
    return undefined;
  }
  let parsed: unknown;
  try {
    parsed = JSON.parse(serialized);
  } catch {
    removeDraft(key);
    return undefined;
  }
  const envelope = draftEnvelopeSchema.safeParse(parsed);
  const value = envelope.success
    ? valueSchema.safeParse(envelope.data.value)
    : undefined;
  if (!envelope.success || !value?.success) {
    removeDraft(key);
    return undefined;
  }
  return {
    savedAt: envelope.data.savedAt,
    source: envelope.data.source,
    value: value.data,
  };
};

const storedDraftSavedAt = (serialized: string | null) => {
  try {
    const envelope = draftEnvelopeSchema.safeParse(
      JSON.parse(serialized ?? ''),
    );
    return envelope.success ? envelope.data.savedAt : '';
  } catch {
    return '';
  }
};

/** Drops other drafts, oldest first, to make room for the current one. */
const pruneOtherDrafts = (target: Storage, keepKey: string) => {
  const drafts = Array.from({ length: target.length }, (_, index) =>
    target.key(index),
  )
    .filter(
      (key): key is string =>
        key !== null &&
        key !== keepKey &&
        key.startsWith(`${draftStorageKeyPrefix}.`),
    )
    .map((key) => ({ key, savedAt: storedDraftSavedAt(target.getItem(key)) }))
    .sort((left, right) => left.savedAt.localeCompare(right.savedAt));
  drafts.forEach(({ key }) => target.removeItem(key));
  return drafts.length > 0;
};

/**
 * Stores a draft, returning whether it was persisted. A draft that cannot be
 * stored replaces any older copy so it is never offered in place of newer
 * edits.
 */
export const writeDraft = <T>(key: string, draft: StoredDraft<T>) => {
  const target = storage();
  if (!target) return false;
  const serialized = JSON.stringify({ format: draftFormatVersion, ...draft });
  if (serialized.length > maxDraftLength) {
    removeDraft(key);
    return false;
  }
  try {
    target.setItem(key, serialized);
    return true;
  } catch {
    try {
      if (pruneOtherDrafts(target, key)) {
        target.setItem(key, serialized);
        return true;
      }
    } catch {
      // Fall through to discard the outdated copy.
    }
    removeDraft(key);
    return false;
  }
};

import type { z } from 'zod';

/** Where a value is kept: across sessions, or for this tab only. */
export const storageAreas = { local: 'local', session: 'session' } as const;
export type StorageArea = (typeof storageAreas)[keyof typeof storageAreas];

// Storage can be unavailable in hardened browser contexts, where even reading
// `localStorage` throws; every helper then behaves as if nothing is stored.
const storageFor = (area: StorageArea) =>
  area === storageAreas.local ? localStorage : sessionStorage;

/** The stored text, or `null` when there is none or storage is unavailable. */
export const readStored = (area: StorageArea, key: string) => {
  try {
    return storageFor(area).getItem(key);
  } catch {
    return null;
  }
};

/** Stores `value`, or removes the key for `null`; ignored without storage. */
export const writeStored = (
  area: StorageArea,
  key: string,
  value: string | null,
) => {
  try {
    if (value === null) storageFor(area).removeItem(key);
    else storageFor(area).setItem(key, value);
  } catch {
    // The caller's state still applies to this page view.
  }
};

/** The stored JSON value when it matches `schema`, otherwise `undefined`. */
export const readStoredJson = <T>(
  area: StorageArea,
  key: string,
  schema: z.ZodType<T>,
): T | undefined => {
  const stored = readStored(area, key);
  if (stored === null) return undefined;
  try {
    const parsed = schema.safeParse(JSON.parse(stored));
    return parsed.success ? parsed.data : undefined;
  } catch {
    return undefined;
  }
};

export const writeStoredJson = (
  area: StorageArea,
  key: string,
  value: unknown,
) => writeStored(area, key, JSON.stringify(value));

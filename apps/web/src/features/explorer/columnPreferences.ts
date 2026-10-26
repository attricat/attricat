import { z } from 'zod';
import {
  readStoredJson,
  writeStored,
  writeStoredJson,
  storageAreas,
} from '../../components/browserStorage';
import { columnPreferencesStorageKeyPrefix } from './constants';

export type ExplorerColumnPreferences = {
  hidden: string[];
  order: string[];
};

const storageKey = (blueprintId: string) =>
  `${columnPreferencesStorageKeyPrefix}${blueprintId}`;

const defaultPreferences = (
  columnIds: string[],
): ExplorerColumnPreferences => ({
  hidden: [],
  order: columnIds,
});

const normalizePreferences = (
  preferences: ExplorerColumnPreferences,
  columnIds: string[],
): ExplorerColumnPreferences => {
  const knownIds = new Set(columnIds);
  const order = [
    ...new Set(preferences.order.filter((id) => knownIds.has(id))),
  ];
  order.push(...columnIds.filter((id) => !order.includes(id)));
  return {
    hidden: [...new Set(preferences.hidden.filter((id) => knownIds.has(id)))],
    order,
  };
};

const preferencesSchema = z.object({
  hidden: z.array(z.string()),
  order: z.array(z.string()),
});

export const getExplorerColumnPreferences = (
  blueprintId: string,
  columnIds: string[],
): ExplorerColumnPreferences => {
  const stored = readStoredJson(
    storageAreas.local,
    storageKey(blueprintId),
    preferencesSchema,
  );
  return stored
    ? normalizePreferences(stored, columnIds)
    : defaultPreferences(columnIds);
};

export const setExplorerColumnPreferences = (
  blueprintId: string,
  preferences: ExplorerColumnPreferences,
) => writeStoredJson(storageAreas.local, storageKey(blueprintId), preferences);

export const clearExplorerColumnPreferences = (blueprintId: string) =>
  writeStored(storageAreas.local, storageKey(blueprintId), null);

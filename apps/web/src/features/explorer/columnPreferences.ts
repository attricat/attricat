import { z } from 'zod';
import {
  readStoredJson,
  writeStored,
  writeStoredJson,
  storageAreas,
} from '../../components/browserStorage';
import {
  columnPreferencesStorageKeyPrefix,
  defaultHiddenColumnIds,
} from './constants';

export type ExplorerColumnPreferences = {
  hidden: string[];
  order: string[];
};

const storageKey = (blueprintId: string) =>
  `${columnPreferencesStorageKeyPrefix}${blueprintId}`;

/** What a viewer without stored preferences sees. */
export const defaultExplorerColumnPreferences = (
  columnIds: string[],
): ExplorerColumnPreferences => ({
  hidden: columnIds.filter((id) => defaultHiddenColumnIds.includes(id)),
  order: columnIds,
});

/**
 * Fits preferences to the table's current columns: unknown ones are dropped
 * and new ones appended as a new viewer would see them.
 */
export const normalizeExplorerColumnPreferences = (
  preferences: ExplorerColumnPreferences,
  columnIds: string[],
): ExplorerColumnPreferences => {
  const knownIds = new Set(columnIds);
  const order = [
    ...new Set(preferences.order.filter((id) => knownIds.has(id))),
  ];
  // Columns added since the preferences were stored start as they would
  // for a new viewer.
  const added = columnIds.filter((id) => !order.includes(id));
  order.push(...added);
  return {
    hidden: [
      ...new Set([
        ...preferences.hidden.filter((id) => knownIds.has(id)),
        ...added.filter((id) => defaultHiddenColumnIds.includes(id)),
      ]),
    ],
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
    ? normalizeExplorerColumnPreferences(stored, columnIds)
    : defaultExplorerColumnPreferences(columnIds);
};

export const setExplorerColumnPreferences = (
  blueprintId: string,
  preferences: ExplorerColumnPreferences,
) => writeStoredJson(storageAreas.local, storageKey(blueprintId), preferences);

export const clearExplorerColumnPreferences = (blueprintId: string) =>
  writeStored(storageAreas.local, storageKey(blueprintId), null);

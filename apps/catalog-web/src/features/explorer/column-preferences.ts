export type ExplorerColumnPreferences = {
  hidden: string[];
  order: string[];
};

const storageKey = (blueprintId: string) =>
  `catalog.explorer.column-preferences.${blueprintId}`;

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
    ...preferences.order.filter((id) => knownIds.has(id)),
    ...columnIds.filter((id) => !preferences.order.includes(id)),
  ];
  return {
    hidden: preferences.hidden.filter((id) => knownIds.has(id)),
    order,
  };
};

const isPreferences = (value: unknown): value is ExplorerColumnPreferences =>
  Boolean(
    value &&
    typeof value === 'object' &&
    Array.isArray((value as ExplorerColumnPreferences).hidden) &&
    Array.isArray((value as ExplorerColumnPreferences).order) &&
    (value as ExplorerColumnPreferences).hidden.every(
      (id) => typeof id === 'string',
    ) &&
    (value as ExplorerColumnPreferences).order.every(
      (id) => typeof id === 'string',
    ),
  );

export const getExplorerColumnPreferences = (
  blueprintId: string,
  columnIds: string[],
): ExplorerColumnPreferences => {
  try {
    const stored = JSON.parse(
      localStorage.getItem(storageKey(blueprintId)) ?? '',
    );
    return isPreferences(stored)
      ? normalizePreferences(stored, columnIds)
      : defaultPreferences(columnIds);
  } catch {
    return defaultPreferences(columnIds);
  }
};

export const setExplorerColumnPreferences = (
  blueprintId: string,
  preferences: ExplorerColumnPreferences,
) => {
  try {
    localStorage.setItem(storageKey(blueprintId), JSON.stringify(preferences));
  } catch {
    // Storage can be unavailable in hardened browser contexts.
  }
};

export const clearExplorerColumnPreferences = (blueprintId: string) => {
  try {
    localStorage.removeItem(storageKey(blueprintId));
  } catch {
    // Storage can be unavailable in hardened browser contexts.
  }
};

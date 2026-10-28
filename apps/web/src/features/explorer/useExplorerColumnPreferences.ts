import { useMemo, useState } from 'react';
import {
  clearExplorerColumnPreferences,
  defaultExplorerColumnPreferences,
  normalizeExplorerColumnPreferences,
  getExplorerColumnPreferences,
  setExplorerColumnPreferences,
  type ExplorerColumnPreferences,
} from './columnPreferences';

/**
 * Column preferences are read once per mounted table; the results table is
 * remounted whenever its rows belong to another blueprint revision. Columns
 * can still appear while it is mounted, so they are fitted to the current
 * columns.
 */
export const useExplorerColumnPreferences = (
  blueprintId: string,
  columnIds: string[],
) => {
  const [preferences, setPreferences] = useState<ExplorerColumnPreferences>(
    () => getExplorerColumnPreferences(blueprintId, columnIds),
  );
  const update = (next: ExplorerColumnPreferences) => {
    setPreferences(next);
    setExplorerColumnPreferences(blueprintId, next);
  };
  const clear = () => {
    clearExplorerColumnPreferences(blueprintId);
    setPreferences(defaultExplorerColumnPreferences(columnIds));
  };
  const columnKey = JSON.stringify(columnIds);
  const current = useMemo(
    () =>
      normalizeExplorerColumnPreferences(
        preferences,
        JSON.parse(columnKey) as string[],
      ),
    [columnKey, preferences],
  );
  return { clear, preferences: current, update };
};

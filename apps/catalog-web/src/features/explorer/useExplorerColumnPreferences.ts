import { useState } from 'react';
import {
  clearExplorerColumnPreferences,
  getExplorerColumnPreferences,
  setExplorerColumnPreferences,
  type ExplorerColumnPreferences,
} from './columnPreferences';

/**
 * Column preferences are read once per mounted table; the results table is
 * remounted whenever the search, and therefore the blueprint, changes.
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
    setPreferences({ hidden: [], order: columnIds });
  };
  return { clear, preferences, update };
};

import { useEffect, useState } from 'react';
import type { EntityItem } from '../entities/api';
import { maximumAgentSelection } from './agent-selection';

export const useExplorerSelection = (items: EntityItem[]) => {
  const [selectionMode, setSelectionMode] = useState(false);
  const [selectedEntityIds, setSelectedEntityIds] = useState<Set<string>>(
    () => new Set(),
  );
  const selectedItems = items.filter((item) => selectedEntityIds.has(item.id));
  const allLoadedSelected =
    items.length > 0 &&
    items
      .slice(0, maximumAgentSelection)
      .every((item) => selectedEntityIds.has(item.id));

  useEffect(() => {
    // Loaded rows can change after a search or refresh; preserve only visible IDs.
    // This synchronizes selection with the externally loaded result set.
    const loaded = new Set(items.map((item) => item.id));
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setSelectedEntityIds((current) => {
      const next = new Set([...current].filter((id) => loaded.has(id)));
      return next.size === current.size ? current : next;
    });
  }, [items]);

  const clearSelection = () => setSelectedEntityIds(new Set());
  const exitSelectionMode = () => {
    setSelectionMode(false);
    clearSelection();
  };
  const toggleSelectionMode = () =>
    selectionMode ? exitSelectionMode() : setSelectionMode(true);
  const toggleLoaded = () =>
    setSelectedEntityIds(
      allLoadedSelected
        ? new Set()
        : new Set(items.slice(0, maximumAgentSelection).map((item) => item.id)),
    );
  const toggleEntity = (entityId: string) =>
    setSelectedEntityIds((current) => {
      const next = new Set(current);
      if (next.has(entityId)) next.delete(entityId);
      else if (next.size < maximumAgentSelection) next.add(entityId);
      return next;
    });
  const removeEntity = (entityId: string) =>
    setSelectedEntityIds((current) => {
      const next = new Set(current);
      next.delete(entityId);
      return next;
    });

  return {
    selectionMode,
    selectedItems,
    allLoadedSelected,
    isSelected: (entityId: string) => selectedEntityIds.has(entityId),
    clearSelection,
    exitSelectionMode,
    toggleSelectionMode,
    toggleLoaded,
    toggleEntity,
    removeEntity,
  };
};

export type ExplorerSelection = ReturnType<typeof useExplorerSelection>;

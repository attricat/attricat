import { useState } from 'react';
import type { EntityItem } from '../entities/api';
import { maximumAgentSelection } from './agentSelection';

type SelectionState = {
  scope: string | undefined;
  selectionMode: boolean;
  entities: EntityItem[];
};

const emptySelection = (scope: string | undefined): SelectionState => ({
  scope,
  selectionMode: false,
  entities: [],
});

/**
 * Tracks bulk selection for one blueprint. Selected entities are kept as
 * snapshots so the selection survives query, sort, and filter changes that
 * replace the loaded rows; switching to another blueprint starts afresh.
 */
export const useExplorerSelection = (
  scope: string | undefined,
  items: EntityItem[],
) => {
  const [state, setState] = useState(() => emptySelection(scope));
  if (state.scope !== scope) setState(emptySelection(scope));
  const current = state.scope === scope ? state : emptySelection(scope);

  const selectedIds = new Set(current.entities.map((entity) => entity.id));
  const loadedById = new Map(items.map((item) => [item.id, item]));
  // Prefer loaded rows so labels and versions reflect the latest results.
  const selectedItems = current.entities.map(
    (entity) => loadedById.get(entity.id) ?? entity,
  );
  const loadedSelectedCount = items.filter((item) =>
    selectedIds.has(item.id),
  ).length;
  const canAddLoaded =
    selectedIds.size < maximumAgentSelection &&
    loadedSelectedCount < items.length;
  const allLoadedSelected = loadedSelectedCount > 0 && !canAddLoaded;
  const someLoadedSelected = loadedSelectedCount > 0 && canAddLoaded;

  const updateEntities = (change: (entities: EntityItem[]) => EntityItem[]) =>
    setState((previous) => ({
      ...previous,
      entities: change(previous.entities),
    }));
  const clearSelection = () => updateEntities(() => []);
  const exitSelectionMode = () => setState(emptySelection(scope));
  const toggleSelectionMode = () =>
    current.selectionMode
      ? exitSelectionMode()
      : setState({ ...current, selectionMode: true });
  const toggleLoaded = () =>
    updateEntities((entities) => {
      const ids = new Set(entities.map((entity) => entity.id));
      const room = maximumAgentSelection - ids.size;
      const addable = items.filter((item) => !ids.has(item.id));
      if (room > 0 && addable.length > 0) {
        return [...entities, ...addable.slice(0, room)];
      }
      return entities.filter((entity) => !loadedById.has(entity.id));
    });
  const toggleEntity = (entity: EntityItem) =>
    updateEntities((entities) => {
      if (entities.some((selected) => selected.id === entity.id)) {
        return entities.filter((selected) => selected.id !== entity.id);
      }
      return entities.length < maximumAgentSelection
        ? [...entities, entity]
        : entities;
    });
  const removeEntity = (entityId: string) =>
    updateEntities((entities) =>
      entities.filter((entity) => entity.id !== entityId),
    );

  return {
    selectionMode: current.selectionMode,
    selectedItems,
    allLoadedSelected,
    someLoadedSelected,
    isSelected: (entityId: string) => selectedIds.has(entityId),
    clearSelection,
    exitSelectionMode,
    toggleSelectionMode,
    toggleLoaded,
    toggleEntity,
    removeEntity,
  };
};

export type ExplorerSelection = ReturnType<typeof useExplorerSelection>;

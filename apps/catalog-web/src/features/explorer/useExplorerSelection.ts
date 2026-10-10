import { useState } from 'react';
import type { RecordItem } from '../records/api';
import { maximumAgentSelection } from './agentSelection';

type SelectionState = {
  scope: string | undefined;
  selectionMode: boolean;
  records: RecordItem[];
};

const emptySelection = (scope: string | undefined): SelectionState => ({
  scope,
  selectionMode: false,
  records: [],
});

/**
 * Tracks bulk selection for one blueprint. Selected records are kept as
 * snapshots so the selection survives query, sort, and filter changes that
 * replace the loaded rows; switching to another blueprint starts afresh.
 */
export const useExplorerSelection = (
  scope: string | undefined,
  items: RecordItem[],
) => {
  const [state, setState] = useState(() => emptySelection(scope));
  if (state.scope !== scope) setState(emptySelection(scope));
  const current = state.scope === scope ? state : emptySelection(scope);

  const selectedIds = new Set(current.records.map((record) => record.id));
  const loadedById = new Map(items.map((item) => [item.id, item]));
  // Prefer loaded rows so labels and versions reflect the latest results.
  const selectedItems = current.records.map(
    (record) => loadedById.get(record.id) ?? record,
  );
  const loadedSelectedCount = items.filter((item) =>
    selectedIds.has(item.id),
  ).length;
  const canAddLoaded =
    selectedIds.size < maximumAgentSelection &&
    loadedSelectedCount < items.length;
  const allLoadedSelected = loadedSelectedCount > 0 && !canAddLoaded;
  const someLoadedSelected = loadedSelectedCount > 0 && canAddLoaded;

  const updateRecords = (change: (records: RecordItem[]) => RecordItem[]) =>
    setState((previous) => ({
      ...previous,
      records: change(previous.records),
    }));
  const clearSelection = () => updateRecords(() => []);
  const exitSelectionMode = () => setState(emptySelection(scope));
  const toggleSelectionMode = () =>
    current.selectionMode
      ? exitSelectionMode()
      : setState({ ...current, selectionMode: true });
  const toggleLoaded = () =>
    updateRecords((records) => {
      const ids = new Set(records.map((record) => record.id));
      const room = maximumAgentSelection - ids.size;
      const addable = items.filter((item) => !ids.has(item.id));
      if (room > 0 && addable.length > 0) {
        return [...records, ...addable.slice(0, room)];
      }
      return records.filter((record) => !loadedById.has(record.id));
    });
  const toggleRecord = (record: RecordItem) =>
    updateRecords((records) => {
      if (records.some((selected) => selected.id === record.id)) {
        return records.filter((selected) => selected.id !== record.id);
      }
      return records.length < maximumAgentSelection
        ? [...records, record]
        : records;
    });
  const removeRecord = (recordId: string) =>
    updateRecords((records) =>
      records.filter((record) => record.id !== recordId),
    );

  return {
    selectionMode: current.selectionMode,
    selectedItems,
    allLoadedSelected,
    someLoadedSelected,
    isSelected: (recordId: string) => selectedIds.has(recordId),
    clearSelection,
    exitSelectionMode,
    toggleSelectionMode,
    toggleLoaded,
    toggleRecord,
    removeRecord,
  };
};

export type ExplorerSelection = ReturnType<typeof useExplorerSelection>;

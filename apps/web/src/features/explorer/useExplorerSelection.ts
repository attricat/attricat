import { useEffect, useState } from 'react';
import {
  readPickerSelection,
  writePickerSelection,
} from './relationshipPickerStorage';
import type { RecordItem } from '../records/api';
import { maximumAgentSelection } from './agentSelection';

type SelectionState = {
  scope: string | undefined;
  selectionMode: boolean;
  records: RecordItem[];
};

type SelectionOptions = {
  /** Start, and stay, in selection mode, as a relationship picker tab does. */
  alwaysSelecting?: boolean;
  /** Most records selected at once; selecting another replaces a single one. */
  maximum?: number;
  /**
   * A relationship picker's token: the selection is kept in the tab's
   * session storage, so it survives creating a record and coming back.
   */
  storageToken?: string;
};

const emptySelection = (
  scope: string | undefined,
  selectionMode = false,
): SelectionState => ({
  scope,
  selectionMode,
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
  {
    alwaysSelecting = false,
    maximum = maximumAgentSelection,
    storageToken,
  }: SelectionOptions = {},
) => {
  const fresh = () => emptySelection(scope, alwaysSelecting);
  const [state, setState] = useState(() => ({
    ...fresh(),
    records: storageToken
      ? readPickerSelection(storageToken).slice(0, maximum)
      : [],
  }));
  useEffect(() => {
    if (storageToken && state.scope === scope)
      writePickerSelection(storageToken, state.records);
  }, [scope, state, storageToken]);
  if (state.scope !== scope) setState(fresh());
  const current = state.scope === scope ? state : fresh();

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
    selectedIds.size < maximum && loadedSelectedCount < items.length;
  // Choosing one record replaces it, so there is nothing to select at once.
  const canSelectLoaded = maximum > 1;
  const allLoadedSelected =
    canSelectLoaded && loadedSelectedCount > 0 && !canAddLoaded;
  const someLoadedSelected =
    canSelectLoaded && loadedSelectedCount > 0 && canAddLoaded;

  const updateRecords = (change: (records: RecordItem[]) => RecordItem[]) =>
    setState((previous) => ({
      ...previous,
      records: change(previous.records),
    }));
  const clearSelection = () => updateRecords(() => []);
  const exitSelectionMode = () => setState(fresh());
  const toggleSelectionMode = () =>
    current.selectionMode
      ? exitSelectionMode()
      : setState({ ...current, selectionMode: true });
  const toggleLoaded = () =>
    canSelectLoaded &&
    updateRecords((records) => {
      const ids = new Set(records.map((record) => record.id));
      const room = maximum - ids.size;
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
      if (maximum === 1) return [record];
      return records.length < maximum ? [...records, record] : records;
    });
  /** Adds records, such as ones created for a picker; a single one replaces. */
  const addRecords = (added: RecordItem[]) =>
    updateRecords((records) => {
      if (added.length === 0) return records;
      if (maximum === 1) return added.slice(-1);
      const ids = new Set(records.map((record) => record.id));
      const additions = added.filter((record) => !ids.has(record.id));
      return [...records, ...additions].slice(0, maximum);
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
    canSelectLoaded,
    isSelected: (recordId: string) => selectedIds.has(recordId),
    clearSelection,
    exitSelectionMode,
    toggleSelectionMode,
    toggleLoaded,
    toggleRecord,
    addRecords,
    removeRecord,
  };
};

export type ExplorerSelection = ReturnType<typeof useExplorerSelection>;

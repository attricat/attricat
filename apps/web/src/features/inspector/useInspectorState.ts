import { useState } from 'react';
import { z } from 'zod';
import {
  readStored,
  readStoredJson,
  storageAreas,
  writeStoredJson,
} from '../../components/browserStorage';
import {
  INSPECTOR_STATE_STORAGE_KEY,
  inspectorPaneIds,
  inspectorPaneOrder,
  type InspectorPaneId,
} from './constants';

type InspectorState = {
  activePane: InspectorPaneId;
  expanded: boolean;
};

const defaultInspectorState: InspectorState = {
  activePane: inspectorPaneIds.server,
  expanded: false,
};

/** Earlier versions persisted only the expanded flag as `"true"`. */
const legacyExpandedValue = 'true';

// Each field falls back on its own, so one stale field keeps the other.
const inspectorStateSchema = z.object({
  activePane: z
    .enum(inspectorPaneOrder)
    .catch(defaultInspectorState.activePane),
  expanded: z.boolean().catch(defaultInspectorState.expanded),
});

const readInspectorState = (): InspectorState =>
  readStored(storageAreas.local, INSPECTOR_STATE_STORAGE_KEY) ===
  legacyExpandedValue
    ? { ...defaultInspectorState, expanded: true }
    : (readStoredJson(
        storageAreas.local,
        INSPECTOR_STATE_STORAGE_KEY,
        inspectorStateSchema,
      ) ?? defaultInspectorState);

/** Inspector visibility and active pane, remembered across page loads. */
export const useInspectorState = () => {
  const [state, setState] = useState(readInspectorState);
  const update = (updates: Partial<InspectorState>) =>
    // Built from the latest state, so updates in one tick all apply; the
    // write is idempotent, so a repeated updater call stores the same value.
    setState((current) => {
      const next = { ...current, ...updates };
      writeStoredJson(storageAreas.local, INSPECTOR_STATE_STORAGE_KEY, next);
      return next;
    });
  return [state, update] as const;
};

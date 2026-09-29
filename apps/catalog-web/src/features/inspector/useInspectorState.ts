import { useState } from 'react';
import {
  INSPECTOR_STATE_STORAGE_KEY,
  inspectorPaneIds,
  inspectorPaneOrder,
  type InspectorPaneId,
} from './constants';

export type InspectorState = {
  activePane: InspectorPaneId;
  expanded: boolean;
};

const defaultInspectorState: InspectorState = {
  activePane: inspectorPaneIds.server,
  expanded: false,
};

/** Earlier versions persisted only the expanded flag as `"true"`. */
const legacyExpandedValue = 'true';

const isInspectorPaneId = (value: unknown): value is InspectorPaneId =>
  inspectorPaneOrder.includes(value as InspectorPaneId);

const readInspectorState = (): InspectorState => {
  try {
    const value = localStorage.getItem(INSPECTOR_STATE_STORAGE_KEY);
    if (value === legacyExpandedValue)
      return { ...defaultInspectorState, expanded: true };
    if (!value) return defaultInspectorState;
    const state = JSON.parse(value) as Partial<InspectorState>;
    return {
      activePane: isInspectorPaneId(state.activePane)
        ? state.activePane
        : defaultInspectorState.activePane,
      expanded:
        typeof state.expanded === 'boolean'
          ? state.expanded
          : defaultInspectorState.expanded,
    };
  } catch {
    return defaultInspectorState;
  }
};

const persistInspectorState = (state: InspectorState) => {
  try {
    localStorage.setItem(INSPECTOR_STATE_STORAGE_KEY, JSON.stringify(state));
  } catch {
    // The Inspector remains usable when storage is unavailable.
  }
};

/** Inspector visibility and active pane, remembered across page loads. */
export const useInspectorState = () => {
  const [state, setState] = useState(readInspectorState);
  const update = (updates: Partial<InspectorState>) => {
    const nextState = { ...state, ...updates };
    setState(nextState);
    persistInspectorState(nextState);
  };
  return [state, update] as const;
};

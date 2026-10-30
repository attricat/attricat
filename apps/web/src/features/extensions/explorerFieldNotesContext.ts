import { createContext, useContext } from 'react';
import type { ExtensionRuntimeScope } from './api';
import type { ActionState, ExtensionNotes } from './fieldNotesData';

export type ExplorerFieldNotes = {
  notes: (recordId: string) => readonly ExtensionNotes[];
  states: (recordId: string) => ReadonlyMap<string, ActionState>;
  runtimeScope: ExtensionRuntimeScope;
  blueprintCode: string;
  contextId: string | null;
};

export const ExplorerFieldNotesContext =
  createContext<ExplorerFieldNotes | null>(null);

/** Action states extensions gave for one Explorer row. */
export const useExplorerActionStates = (recordId: string) =>
  useContext(ExplorerFieldNotesContext)?.states(recordId);

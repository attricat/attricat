import { createContext, useContext } from 'react';
import type { DefinitionTarget } from './definitionNavigation';

/** Shows a place in the definition being edited, beside its preview. */
export type RevealDefinition = (target: DefinitionTarget) => void;

/**
 * Supplied by the definition workspace to its preview, so the preview can
 * lead to what it renders. Previews of saved definitions have none.
 */
export const DefinitionRevealContext = createContext<RevealDefinition | null>(
  null,
);

export const useRevealDefinition = () => useContext(DefinitionRevealContext);

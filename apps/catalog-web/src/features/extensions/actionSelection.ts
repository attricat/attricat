import { createContext, useContext } from 'react';
import { z } from 'zod';
import type { ExtensionContribution } from './api';
import {
  maximumActionSelection,
  selectionActionContributionVersion,
  selectionActionOutlets,
  selectionContextVersion,
  selectionSources,
} from './constants';

export type SelectionSource =
  (typeof selectionSources)[keyof typeof selectionSources];

/**
 * Saved entities an action applies to, in display order. One blueprint
 * revision only; the host never widens it to hidden rows or search results.
 */
export type ActionSelection = {
  source: SelectionSource;
  blueprintId: string;
  blueprintVersion: number;
  /** The effective value-resolution context, when the surface has one. */
  contextId: string | null;
  entityIds: readonly string[];
};

/** The strict, versioned context a selection-aware contribution receives. */
export const selectionContextSchema = z
  .object({
    context_version: z.literal(selectionContextVersion),
    selection_source: z.enum([
      selectionSources.entityPreview,
      selectionSources.explorerRow,
      selectionSources.explorerSelection,
    ]),
    blueprint_id: z.uuid(),
    blueprint_version: z.number().int().positive(),
    context_id: z.uuid().nullable(),
    entity_ids: z
      .array(z.uuid())
      .min(1)
      .max(maximumActionSelection)
      .refine((ids) => new Set(ids).size === ids.length),
  })
  .strict();
export type SelectionContext = z.infer<typeof selectionContextSchema>;

export const selectionContext = (
  selection: ActionSelection,
): SelectionContext => ({
  context_version: selectionContextVersion,
  selection_source: selection.source,
  blueprint_id: selection.blueprintId,
  blueprint_version: selection.blueprintVersion,
  context_id: selection.contextId,
  entity_ids: [...selection.entityIds],
});

/** Parses a frame context, returning the selection only for version 2. */
export const parseSelectionContext = (context: unknown) => {
  const result = selectionContextSchema.safeParse(context);
  return result.success ? result.data : undefined;
};

export const isSelectionContribution = (contribution: ExtensionContribution) =>
  contribution.version === selectionActionContributionVersion &&
  (selectionActionOutlets as readonly (string | null)[]).includes(
    contribution.outlet,
  );

/**
 * Supplied by a selection-aware outlet. Version 2 contributions mounted below
 * it receive the selection context; version 1 keeps its released context.
 */
export const ActionSelectionContext = createContext<ActionSelection | null>(
  null,
);

export const useActionSelection = () => useContext(ActionSelectionContext);

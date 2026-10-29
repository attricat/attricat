import { createContext, useContext } from 'react';
import { z } from 'zod';
import type { ExtensionContribution } from './api';
import {
  actionContextVersion,
  actionOutlets,
  maximumActionSelection,
  maximumNotificationMessageLength,
  notificationSeverities,
  type ActionOutlet,
} from './constants';

/**
 * The saved records an action applies to, in display order. One blueprint
 * revision only; the host never widens it to hidden rows or search results.
 * A revision-wide surface, such as the Explorer toolbar, names no records.
 */
export type ActionTarget = {
  blueprintId: string;
  blueprintVersion: number;
  /** The effective value-resolution context, when the surface has one. */
  contextId: string | null;
  recordIds: readonly string[];
  /** The field a field-note badge's action was chosen for. */
  attributeCode?: string;
};

/** What an action's dialog or command receives. */
export const actionContextSchema = z
  .object({
    context_version: z.literal(actionContextVersion),
    action_id: z.string().min(1),
    outlet: z.enum(actionOutlets),
    blueprint_id: z.uuid(),
    blueprint_version: z.number().int().positive(),
    context_id: z.uuid().nullable(),
    record_ids: z
      .array(z.uuid())
      .max(maximumActionSelection)
      .refine((ids) => new Set(ids).size === ids.length),
    attribute_code: z.string().min(1).optional(),
  })
  .strict();
export type ActionContext = z.infer<typeof actionContextSchema>;

export const actionContext = (
  action: Pick<ExtensionContribution, 'id'>,
  outlet: ActionOutlet,
  target: ActionTarget,
): ActionContext => ({
  context_version: actionContextVersion,
  action_id: action.id,
  outlet,
  blueprint_id: target.blueprintId,
  blueprint_version: target.blueprintVersion,
  context_id: target.contextId,
  record_ids: [...target.recordIds],
  ...(target.attributeCode !== undefined && {
    attribute_code: target.attributeCode,
  }),
});

/** Parses a frame context, returning it only when it is an action context. */
export const parseActionContext = (context: unknown) => {
  const result = actionContextSchema.safeParse(context);
  return result.success ? result.data : undefined;
};

/**
 * What the host does with an action command's response. Other fields are the
 * command's own and are ignored.
 */
export const actionResultSchema = z.object({
  notification: z
    .object({
      message: z.string().min(1).max(maximumNotificationMessageLength),
      severity: z.enum(notificationSeverities).optional(),
    })
    .optional(),
  /** Reload host views of the records the action applied to. */
  refresh: z.boolean().optional(),
});

/** Closes the menu an action item sits in, once the item is chosen. */
export const ActionMenuCloseContext = createContext<(() => void) | null>(null);

export const useCloseActionMenu = () => useContext(ActionMenuCloseContext);

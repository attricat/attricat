import type { ActionTarget } from './actions';
import type { ExtensionContribution } from './api';
import { contributionKinds, extensionOutlets } from './constants';
import { actionStateKey, type ActionState } from './fieldNotesData';

export type ActionAvailability =
  { offered: false } | { offered: true; disabledReason?: string };

/**
 * Whether the host offers an action for a target: its declarative `when`,
 * then the state its extension gave for the record. A condition the target
 * cannot tell (such as tags the surface does not know) does not hide the
 * action; the server still enforces `requires_write`. A command runs only for
 * callers who may write records, so it is not offered to the others.
 */
export const actionAvailability = (
  action: Pick<ExtensionContribution, 'extension_id' | 'id' | 'when'> &
    Partial<Pick<ExtensionContribution, 'invoke'>>,
  target: ActionTarget,
  states?: ReadonlyMap<string, ActionState>,
): ActionAvailability => {
  const when = action.when;
  const count = target.recordIds.length;
  const tags = target.recordTags;
  const hidden =
    (target.canRunCommands === false &&
      action.invoke &&
      'command' in action.invoke) ||
    (when?.blueprints?.length &&
      target.blueprintCode !== undefined &&
      !when.blueprints.includes(target.blueprintCode)) ||
    (when?.min_records !== undefined && count < when.min_records) ||
    (when?.max_records !== undefined && count > when.max_records) ||
    (when?.requires_write && target.canWrite === false) ||
    (tags !== undefined &&
      ((when?.has_tags ?? []).some((tag) =>
        tags.some((recordTags) => !recordTags.includes(tag)),
      ) ||
        (when?.missing_tags ?? []).some((tag) =>
          tags.some((recordTags) => recordTags.includes(tag)),
        )));
  if (hidden) return { offered: false };
  const state = states?.get(actionStateKey(action.extension_id, action.id));
  if (state && 'hidden' in state) return { offered: false };
  return state
    ? { offered: true, disabledReason: state.disabled }
    : { offered: true };
};

/**
 * The badge-only action a badge names, when the host would offer it for the
 * target and its extension has not hidden or disabled it for the record.
 */
export const offeredBadgeAction = (
  contributions: readonly ExtensionContribution[] | undefined,
  extensionId: string,
  actionId: string | undefined,
  target: ActionTarget,
  states: ReadonlyMap<string, ActionState>,
) => {
  if (actionId === undefined) return undefined;
  const action = contributions?.find(
    (item) =>
      item.extension_id === extensionId &&
      item.id === actionId &&
      item.kind === contributionKinds.action &&
      item.outlet === extensionOutlets.recordAttributeAction,
  );
  if (!action) return undefined;
  const availability = actionAvailability(action, target, states);
  return availability.offered && !availability.disabledReason
    ? action
    : undefined;
};

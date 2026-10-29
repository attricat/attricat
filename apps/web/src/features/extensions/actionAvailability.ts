import type { ActionTarget } from './actions';
import type { ExtensionContribution } from './api';
import { actionStateKey, type ActionState } from './fieldNotesData';

export type ActionAvailability =
  { offered: false } | { offered: true; disabledReason?: string };

/**
 * Whether the host offers an action for a target: its declarative `when`,
 * then the state its extension gave for the record. A condition the target
 * cannot tell (such as tags the surface does not know) does not hide the
 * action; the server still enforces `requires_write`.
 */
export const actionAvailability = (
  action: Pick<ExtensionContribution, 'extension_id' | 'id' | 'when'>,
  target: ActionTarget,
  states?: ReadonlyMap<string, ActionState>,
): ActionAvailability => {
  const when = action.when;
  const count = target.recordIds.length;
  const tags = target.recordTags;
  const hidden =
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

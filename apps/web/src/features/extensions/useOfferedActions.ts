import { capabilities } from '../auth/capabilities';
import { useCan } from '../auth/useCan';
import {
  actionAvailability,
  type ActionAvailability,
} from './actionAvailability';
import type { ActionTarget } from './actions';
import type { ExtensionContribution, ExtensionRuntimeScope } from './api';
import {
  contributionKinds,
  extensionOutlets,
  type ActionOutlet,
} from './constants';
import {
  actionStates,
  useRecordFieldNotes,
  type ActionState,
} from './fieldNotesData';
import { useExtensionRuntime } from './useExtensionRuntime';

export type ActionsProps = {
  outlet: ActionOutlet;
  runtimeScope: ExtensionRuntimeScope;
  target: ActionTarget;
  /** Action states extensions gave, when the surface already has them. */
  states?: ReadonlyMap<string, ActionState>;
};

export type OfferedAction = {
  action: ExtensionContribution;
  availability: Extract<ActionAvailability, { offered: true }>;
};

/** Record-page outlets, whose actions read their record's field notes. */
const recordOutlets = new Set<ActionOutlet>([
  extensionOutlets.recordAction,
  extensionOutlets.recordHeaderAction,
]);

/**
 * The actions an outlet offers for a target, in host-computed order, with
 * any disabled reason. Record-page actions read the states their extensions
 * gave in the record's field notes.
 */
export const useOfferedActions = ({
  outlet,
  runtimeScope,
  states,
  target,
}: ActionsProps): OfferedAction[] => {
  const runtime = useExtensionRuntime(runtimeScope);
  const canRunCommands = useCan(capabilities.recordsWrite);
  const recordId =
    recordOutlets.has(outlet) && target.recordIds.length === 1
      ? target.recordIds[0]
      : undefined;
  const notes = useRecordFieldNotes(recordId, target.contextId);
  const recordStates =
    states ?? (recordId ? actionStates(notes.data, recordId) : undefined);
  return (runtime.data ?? []).flatMap((action) => {
    if (
      action.kind !== contributionKinds.action ||
      action.outlet !== outlet ||
      action.invoke === null
    )
      return [];
    const availability = actionAvailability(
      action,
      { ...target, canRunCommands },
      recordStates,
    );
    return availability.offered ? [{ action, availability }] : [];
  });
};

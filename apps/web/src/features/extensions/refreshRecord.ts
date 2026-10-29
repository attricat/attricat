import type { QueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import type { ExtensionContribution } from './api';
import { parseActionContext } from './actions';
import {
  extensionCapabilities,
  extensionProtocolErrors,
  extensionOutlets,
} from './constants';
import { recordQueryKeys } from '../records/queryKeys';
import { ruleQueryKeys } from '../rules/queryKeys';

const refreshSchema = z
  .object({ target: z.literal('current_record') })
  .strict();
const recordRefreshOutlets = new Set<ExtensionContribution['outlet']>([
  extensionOutlets.actionDialog,
  extensionOutlets.recordPreviewPanel,
]);

/** Action outlets on a record page, whose dialogs act on that one record. */
const recordActionOutlets = new Set<string>([
  extensionOutlets.recordAction,
  extensionOutlets.recordHeaderAction,
]);

/**
 * The record a host-supplied context describes. A dialog receives its
 * action's context instead, which names the current record only when the
 * action was on a record page; an Explorer selection names none.
 */
const contextRecordId = (context: Record<string, unknown>) => {
  if (context.record_id !== undefined) return z.uuid().parse(context.record_id);
  const action = parseActionContext(context);
  if (
    action &&
    recordActionOutlets.has(action.outlet) &&
    action.record_ids.length === 1
  )
    return action.record_ids[0];
  throw new Error(extensionProtocolErrors.refreshDenied);
};

/** Check the host-supplied outlet and context; never trust a record ID from the frame. */
export const refreshCurrentRecord = async (
  client: QueryClient,
  contribution: Pick<ExtensionContribution, 'capabilities' | 'outlet'>,
  context: Record<string, unknown>,
  payload: unknown,
) => {
  if (
    !contribution.capabilities.includes(extensionCapabilities.refresh) ||
    contribution.outlet === null ||
    !recordRefreshOutlets.has(contribution.outlet)
  )
    throw new Error(extensionProtocolErrors.refreshDenied);
  refreshSchema.parse(payload);
  await refreshRecord(client, contextRecordId(context));
};

/** Refresh host-owned views of one record without exposing cache keys to frames. */
export const refreshRecord = async (client: QueryClient, recordId: string) => {
  const keys = [
    recordQueryKeys.form(recordId),
    recordQueryKeys.changes(recordId),
    recordQueryKeys.recordControls(recordId),
    recordQueryKeys.publication(recordId),
    recordQueryKeys.resolvedPreviews(recordId),
    recordQueryKeys.hierarchies(recordId),
    ruleQueryKeys.findings(recordId),
  ];
  await Promise.all(
    keys.map((queryKey) =>
      client.invalidateQueries({ queryKey }, { throwOnError: true }),
    ),
  );
};

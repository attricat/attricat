import type { QueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import type { ExtensionContribution } from './api';
import {
  extensionCapabilities,
  extensionProtocolErrors,
  selectionSources,
} from './constants';
import { recordQueryKeys } from '../records/queryKeys';
import { ruleQueryKeys } from '../rules/queryKeys';

const refreshSchema = z
  .object({ target: z.literal('current_record') })
  .strict();
const recordRefreshOutlets = new Set([
  'record_action',
  'record_preview_panel',
  'record_attribute_decoration',
]);

/**
 * The record a host-supplied context describes. A version 2 record action
 * receives a selection context instead, whose preview selection holds exactly
 * that one record; any other selection names no single current record.
 */
const contextRecordId = (context: Record<string, unknown>) => {
  if (context.record_id !== undefined) return z.uuid().parse(context.record_id);
  if (
    context.selection_source === selectionSources.recordPreview &&
    Array.isArray(context.record_ids) &&
    context.record_ids.length === 1
  )
    return z.uuid().parse(context.record_ids[0]);
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
    recordQueryKeys.preview(recordId),
    recordQueryKeys.changes(recordId),
    recordQueryKeys.recordControls(recordId),
    recordQueryKeys.publication(recordId),
    recordQueryKeys.resolvedPreviews(recordId),
    recordQueryKeys.hierarchies(recordId),
    recordQueryKeys.incomingRelationshipResults(recordId),
    ruleQueryKeys.findings(recordId),
  ];
  await Promise.all(
    keys.map((queryKey) =>
      client.invalidateQueries({ queryKey }, { throwOnError: true }),
    ),
  );
};

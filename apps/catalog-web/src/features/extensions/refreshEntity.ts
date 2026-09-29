import type { QueryClient } from '@tanstack/react-query';
import { z } from 'zod';
import type { ExtensionContribution } from './api';
import { extensionCapabilities, extensionProtocolErrors } from './constants';
import { entityQueryKeys } from '../entities/queryKeys';
import { ruleQueryKeys } from '../rules/queryKeys';

const refreshSchema = z
  .object({ target: z.literal('current_entity') })
  .strict();
const entityRefreshOutlets = new Set([
  'entity_action',
  'entity_preview_panel',
  'entity_attribute_decoration',
]);

/** Check the host-supplied outlet and context; never trust an entity ID from the frame. */
export const refreshCurrentEntity = async (
  client: QueryClient,
  contribution: Pick<ExtensionContribution, 'capabilities' | 'outlet'>,
  context: Record<string, unknown>,
  payload: unknown,
) => {
  if (
    !contribution.capabilities.includes(extensionCapabilities.refresh) ||
    contribution.outlet === null ||
    !entityRefreshOutlets.has(contribution.outlet)
  )
    throw new Error(extensionProtocolErrors.refreshDenied);
  refreshSchema.parse(payload);
  const entityId = z.uuid().parse(context.entity_id);
  await refreshEntity(client, entityId);
};

/** Refresh host-owned views of one entity without exposing cache keys to frames. */
export const refreshEntity = async (client: QueryClient, entityId: string) => {
  const keys = [
    entityQueryKeys.form(entityId),
    entityQueryKeys.preview(entityId),
    entityQueryKeys.changes(entityId),
    entityQueryKeys.publication(entityId),
    entityQueryKeys.resolvedPreviews(entityId),
    entityQueryKeys.hierarchies(entityId),
    entityQueryKeys.incomingRelationshipResults(entityId),
    ruleQueryKeys.findings(entityId),
  ];
  await Promise.all(
    keys.map((queryKey) =>
      client.invalidateQueries({ queryKey }, { throwOnError: true }),
    ),
  );
};

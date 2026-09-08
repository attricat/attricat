import { z } from 'zod';
import { request, requestText } from '../../api/request';
import { maximumExtensionResponseBytes } from './constants';

const contributionSchema = z
  .object({
    extension_id: z.string().min(1),
    release_id: z.uuid(),
    configuration: z.unknown(),
    capabilities: z.array(z.string()),
    id: z.string().min(1),
    version: z.number().int().positive(),
    kind: z.enum(['route', 'embedded', 'action', 'panel']),
    outlet: z
      .enum([
        'navigation',
        'entity_preview_panel',
        'blueprint_attribute_configuration',
        'entity_attribute_decoration',
        'entity_action',
        'explorer_row_action',
        'blueprint_detail_panel',
        'explorer_action',
        'explorer_bulk_action',
        'entity_header_action',
        'entity_attribute_panel',
        'blueprint_panel',
        'blueprint_publish_check',
        'file_panel',
        'audit_event_panel',
        'data_health_card',
      ])
      .nullable(),
    title: z.string().nullable(),
  })
  .strict();

export const runtimeSchema = z.array(contributionSchema);
export type ExtensionContribution = z.infer<typeof contributionSchema>;

export const getExtensionRuntime = () =>
  request('/api/extensions/runtime', runtimeSchema);

export const getExtensionArtifact = async (
  extensionId: string,
  contributionId: string,
) =>
  requestText(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/artifact`,
  );

export const extensionCommandRequestSchema = z
  .object({
    release_id: z.uuid(),
    command_id: z.string().min(1).max(128),
    payload: z.unknown(),
  })
  .strict();

export const extensionStorageRequestSchema = z.discriminatedUnion('operation', [
  z.object({ operation: z.literal('get'), key: z.string() }).strict(),
  z
    .object({
      operation: z.literal('set'),
      key: z.string(),
      value: z.unknown(),
      expected_revision: z.number().int().positive().optional(),
    })
    .strict(),
  z
    .object({
      operation: z.literal('delete'),
      key: z.string(),
      expected_revision: z.number().int().positive().optional(),
    })
    .strict(),
  z
    .object({
      operation: z.literal('list'),
      prefix: z.string().optional(),
      cursor: z.string().optional(),
      limit: z.number().int().min(1).max(100).optional(),
    })
    .strict(),
]);
export type ExtensionStorageRequest = z.infer<
  typeof extensionStorageRequestSchema
>;

const parseExtensionResponse = (
  text: string,
  operation: 'command' | 'storage',
): unknown => {
  if (new TextEncoder().encode(text).length > maximumExtensionResponseBytes)
    throw new Error(`Extension ${operation} response is too large`);
  try {
    return JSON.parse(text) as unknown;
  } catch (error) {
    throw new Error(`Extension ${operation} response contains malformed JSON`, {
      cause: error,
    });
  }
};

export const extensionCommand = async (
  extensionId: string,
  contributionId: string,
  input: z.infer<typeof extensionCommandRequestSchema>,
) => {
  const response = await requestText(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/command`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    },
  );
  return parseExtensionResponse(response, 'command');
};

export const extensionStorage = async (
  extensionId: string,
  contributionId: string,
  releaseId: string,
  input: ExtensionStorageRequest,
) => {
  const response = await requestText(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/storage/${encodeURIComponent(releaseId)}`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    },
  );
  return parseExtensionResponse(response, 'storage');
};

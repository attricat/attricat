import { z } from 'zod';
import { apiFetch } from '../auth/request';
import { maximumExtensionResponseBytes } from './constants';

const contributionSchema = z
  .object({
    extension_id: z.string().min(1),
    release_id: z.uuid(),
    configuration: z.unknown(),
    capabilities: z.array(z.string()),
    id: z.string().min(1),
    version: z.number().int().positive(),
    kind: z.enum(['route', 'element', 'action', 'panel']),
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
    element: z.string().min(1),
  })
  .strict();

export const runtimeSchema = z.array(contributionSchema);
export type ExtensionContribution = z.infer<typeof contributionSchema>;

const request = async (path: string) => {
  const response = await apiFetch(path);
  if (!response.ok) throw new Error('Extension runtime is unavailable');
  return response;
};

export const getExtensionRuntime = async () =>
  runtimeSchema.parse(await (await request('/api/extensions/runtime')).json());

export const getExtensionArtifact = async (
  extensionId: string,
  contributionId: string,
) =>
  (
    await request(
      `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/artifact`,
    )
  ).text();

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

export const extensionCommand = async (
  extensionId: string,
  contributionId: string,
  input: z.infer<typeof extensionCommandRequestSchema>,
) => {
  const response = await apiFetch(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/command`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    },
  );
  if (!response.ok) throw new Error('Extension command was denied');
  return response.json() as Promise<unknown>;
};

export const extensionStorage = async (
  extensionId: string,
  contributionId: string,
  releaseId: string,
  input: ExtensionStorageRequest,
) => {
  const response = await apiFetch(
    `/api/extensions/${encodeURIComponent(extensionId)}/${encodeURIComponent(contributionId)}/storage/${encodeURIComponent(releaseId)}`,
    {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(input),
    },
  );
  if (!response.ok) throw new Error('Extension storage request was denied');
  const text = await response.text();
  if (new TextEncoder().encode(text).length > maximumExtensionResponseBytes)
    throw new Error('Extension storage response is too large');
  return JSON.parse(text) as unknown;
};

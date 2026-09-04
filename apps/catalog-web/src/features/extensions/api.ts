import { z } from 'zod';
import { apiFetch } from '../auth/request';

const contributionSchema = z
  .object({
    extension_id: z.string().min(1),
    release_id: z.uuid(),
    configuration: z.unknown(),
    capabilities: z.array(z.string()),
    id: z.string().min(1),
    version: z.number().int().positive(),
    kind: z.enum(['route', 'element']),
    outlet: z.enum(['navigation', 'entity_preview_panel']).nullable(),
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

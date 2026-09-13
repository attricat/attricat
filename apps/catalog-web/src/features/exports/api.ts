import { z } from 'zod';
import { request } from '../../api/request';

const publicationChannelSchema = z.object({
  context_id: z.uuid(),
  context_code: z.string(),
  enabled: z.boolean(),
});

export type PublicationChannel = z.infer<typeof publicationChannelSchema>;

export const listPublicationChannels = () =>
  request('/api/publication-channels', z.array(publicationChannelSchema));

export const updatePublicationChannel = (contextId: string, enabled: boolean) =>
  request(
    `/api/publication-channels/${encodeURIComponent(z.uuid().parse(contextId))}`,
    publicationChannelSchema,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled }),
    },
  );

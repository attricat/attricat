import { z } from 'zod';
import { request } from '../../api/request';
import { PUBLICATION_CHANNELS_PATH } from './constants';

const publicationChannelSchema = z.object({
  context_id: z.uuid(),
  context_code: z.string(),
  enabled: z.boolean(),
});

export type PublicationChannel = z.infer<typeof publicationChannelSchema>;

export const listPublicationChannels = () =>
  request(PUBLICATION_CHANNELS_PATH, z.array(publicationChannelSchema));

export const updatePublicationChannel = (contextId: string, enabled: boolean) =>
  request(
    `${PUBLICATION_CHANNELS_PATH}/${encodeURIComponent(z.uuid().parse(contextId))}`,
    publicationChannelSchema,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled }),
    },
  );

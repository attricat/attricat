import { z } from 'zod';
import { request } from '../../api/request';
import { PUBLICATION_CHANNELS_PATH } from './constants';

const publicationChannelSchema = z.object({
  context_id: z.uuid(),
  context_code: z.string(),
  enabled: z.boolean(),
  /** Codes of enabled rules that must pass before publication. */
  required_rule_codes: z.array(z.string()).default([]),
  /** Whether the record schema and blueprint checks must pass. */
  require_valid_record: z.boolean().default(false),
});

/** Publication checks of a channel; omitted settings are kept by the API. */
export type PublicationChannelChecks = {
  required_rule_codes?: string[];
  require_valid_record?: boolean;
};

export type PublicationChannel = z.infer<typeof publicationChannelSchema>;

export const listPublicationChannels = () =>
  request(PUBLICATION_CHANNELS_PATH, z.array(publicationChannelSchema));

export const updatePublicationChannel = (
  contextId: string,
  enabled: boolean,
  checks: PublicationChannelChecks = {},
) =>
  request(
    `${PUBLICATION_CHANNELS_PATH}/${encodeURIComponent(z.uuid().parse(contextId))}`,
    publicationChannelSchema,
    {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ enabled, ...checks }),
    },
  );

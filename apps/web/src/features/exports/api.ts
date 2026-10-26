import { z } from 'zod';
import { request, jsonRequest } from '../../api/request';
import { PUBLICATION_CHANNELS_PATH } from './constants';
import { uuidSegment } from '../../api/pathSegments';
import {
  publicationChannelSchema,
  type PublicationChannelChecks,
} from './schemas';
export type { PublicationChannelChecks, PublicationChannel } from './schemas';

export const listPublicationChannels = () =>
  request(PUBLICATION_CHANNELS_PATH, z.array(publicationChannelSchema));

export const updatePublicationChannel = (
  contextId: string,
  enabled: boolean,
  checks: PublicationChannelChecks = {},
) =>
  request(
    `${PUBLICATION_CHANNELS_PATH}/${uuidSegment(contextId)}`,
    publicationChannelSchema,
    jsonRequest('PUT', { enabled, ...checks }),
  );

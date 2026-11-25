import { z } from 'zod';
import { jsonRequest, request, requestNoContent } from '../../api/request';
import { extensionPath } from '../extensions/apiPaths';
import { issuedWebhookEndpointSchema, webhookEndpointSchema } from './schemas';

const endpointsPath = (extensionId: string) =>
  `${extensionPath(extensionId)}/webhook-endpoints`;

const endpointPath = (extensionId: string, endpoint: string) =>
  `${endpointsPath(extensionId)}/${encodeURIComponent(endpoint)}`;

export const listWebhookEndpoints = (extensionId: string) =>
  request(endpointsPath(extensionId), z.array(webhookEndpointSchema));

export const createWebhookEndpoint = (
  extensionId: string,
  input: { webhook: string; endpoint: string },
) =>
  request(
    endpointsPath(extensionId),
    issuedWebhookEndpointSchema,
    jsonRequest('POST', input),
  );

/** Issues a new URL; the previous one stops working at once. */
export const rotateWebhookEndpoint = (extensionId: string, endpoint: string) =>
  request(
    `${endpointPath(extensionId, endpoint)}/rotate`,
    issuedWebhookEndpointSchema,
    { method: 'POST' },
  );

export const deleteWebhookEndpoint = (extensionId: string, endpoint: string) =>
  requestNoContent(endpointPath(extensionId, endpoint), { method: 'DELETE' });

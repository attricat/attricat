import { queryOptions } from '@tanstack/react-query';
import { listWebhookEndpoints } from './api';
import { extensionWebhookQueryKeys } from './queryKeys';

/** The public webhook endpoints of one installed extension. */
export const webhookEndpointsOptions = (extensionId: string) =>
  queryOptions({
    queryKey: extensionWebhookQueryKeys.endpoints(extensionId),
    queryFn: () => listWebhookEndpoints(extensionId),
  });

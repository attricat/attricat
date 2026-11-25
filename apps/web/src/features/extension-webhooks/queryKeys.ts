const root = ['extension-webhooks'] as const;

export const extensionWebhookQueryKeys = {
  all: root,
  endpoints: (extensionId: string) =>
    [...root, 'endpoints', extensionId] as const,
  /** Every mutation on one endpoint, so its row knows when one runs. */
  endpointMutations: (extensionId: string, endpoint: string) =>
    [...root, 'endpoints', extensionId, endpoint, 'mutations'] as const,
};

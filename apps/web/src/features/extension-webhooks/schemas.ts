import { z } from 'zod';

/** One public endpoint an operator created for a declared webhook. */
export const webhookEndpointSchema = z.object({
  id: z.uuid(),
  extension_id: z.string(),
  /** The manifest webhook it delivers to. */
  webhook: z.string(),
  /** The operator's name for it. */
  endpoint: z.string(),
  created_at: z.string(),
  token_rotated_at: z.string().nullable(),
  last_request_at: z.string().nullable(),
  last_reply_status: z.number().int().nullable(),
});
export type WebhookEndpoint = z.infer<typeof webhookEndpointSchema>;

/**
 * An endpoint with the URL just issued for it, which no later response
 * carries again. `url` is set only when the server knows its public address.
 */
export const issuedWebhookEndpointSchema = webhookEndpointSchema.extend({
  path: z.string(),
  url: z.string().nullable(),
});
export type IssuedWebhookEndpoint = z.infer<typeof issuedWebhookEndpointSchema>;

/** The webhooks an installed release declares. */
export const manifestWebhooksSchema = z.looseObject({
  server: z
    .looseObject({
      webhooks: z
        .array(
          z.looseObject({
            id: z.string(),
            title: z.string(),
          }),
        )
        .default([]),
    })
    .nullable()
    .optional(),
});
export type ManifestWebhook = NonNullable<
  z.infer<typeof manifestWebhooksSchema>['server']
>['webhooks'][number];

/** The webhooks a manifest declares; none when it cannot be read. */
export const manifestWebhooks = (manifest: unknown): ManifestWebhook[] =>
  manifestWebhooksSchema.safeParse(manifest).data?.server?.webhooks ?? [];

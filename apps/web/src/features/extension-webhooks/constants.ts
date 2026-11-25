/** Longest endpoint name the API accepts. */
export const maximumWebhookEndpointNameLength = 128;

/**
 * Endpoint names the API accepts: letters, digits, `.`, `_` or `-`, at most
 * `maximumWebhookEndpointNameLength` of them.
 */
export const webhookEndpointNamePattern = new RegExp(
  `^[A-Za-z0-9._-]{1,${maximumWebhookEndpointNameLength}}$`,
);

/** `details.resource` of a `code_taken` error for an endpoint name in use. */
export const webhookEndpointResource = 'webhook_endpoint';

/**
 * A push error an extension recorded, as `code: message`. It is the
 * extension's own report, not a failed request, so it is shown as written.
 */
export const formatPushError = (
  error: { code?: string | null; message?: string | null } | null | undefined,
) => [error?.code, error?.message].filter(Boolean).join(': ');

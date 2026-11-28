import { z } from 'zod';
import { handlerFailureSchema } from '../extension-exports/schemas';

export const runFailureStates = {
  /** Dead-lettered: it runs again only when someone replays it. */
  failed: 'failed',
  retrying: 'retrying',
} as const;

export type RunFailureState =
  (typeof runFailureStates)[keyof typeof runFailureStates];

/** A run that failed, or waits to retry after a failure. */
export const runFailureSchema = z.object({
  id: z.uuid(),
  operation_id: z.string(),
  state: z.enum(runFailureStates),
  /** A schedule started the run, not a person. */
  scheduled: z.boolean(),
  attempts: z.number().int(),
  error_code: z.string().nullable(),
  error_message: z.string().nullable(),
  updated_at: z.string(),
});
export type RunFailure = z.infer<typeof runFailureSchema>;

export const runFailurePageSchema = z.object({
  items: z.array(runFailureSchema),
  has_more: z.boolean(),
});

export const deliveryFailurePageSchema = z.object({
  items: z.array(handlerFailureSchema),
  has_more: z.boolean(),
});

/** The server sections whose work can fail in the background. */
export const manifestBackgroundWorkSchema = z.looseObject({
  server: z
    .looseObject({
      event_handlers: z.array(z.unknown()).default([]),
      operations: z.array(z.unknown()).default([]),
    })
    .nullable()
    .optional(),
});

/** Whether the extension declares event handlers or operations. */
export const hasBackgroundWork = (manifest: unknown) => {
  const server = manifestBackgroundWorkSchema.safeParse(manifest).data?.server;
  return (
    !!server && server.event_handlers.length + server.operations.length > 0
  );
};

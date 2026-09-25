import { z } from 'zod';

export const backgroundProcessingStatusSchema = z.array(
  z.object({
    kind: z.string(),
    queued: z.number().int().nonnegative(),
    running: z.number().int().nonnegative(),
    failed: z.number().int().nonnegative(),
    expired_leases: z.number().int().nonnegative(),
    oldest_due_seconds: z.number().nonnegative().nullable(),
  }),
);

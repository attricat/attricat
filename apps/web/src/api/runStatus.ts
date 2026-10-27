import { z } from 'zod';

/**
 * The lifecycle of a queued rule or workflow run: it waits, is leased by a
 * worker, and completes, is cancelled or exhausts its attempts.
 */
export const runStatuses = {
  pending: 'pending',
  leased: 'leased',
  completed: 'completed',
  deadLetter: 'dead_letter',
  cancelled: 'cancelled',
} as const;

export const runStatusSchema = z.enum(runStatuses);

export type RunStatus = z.infer<typeof runStatusSchema>;

/** Run statuses that have not finished. */
export const activeRunStatuses: readonly RunStatus[] = [
  runStatuses.pending,
  runStatuses.leased,
];

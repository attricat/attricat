import { z } from 'zod';

export const systemHealthSchema = z.object({
  build: z.object({
    version: z.string(),
    branch: z.string(),
    commit: z.string(),
  }),
});

export type SystemHealth = z.infer<typeof systemHealthSchema>;

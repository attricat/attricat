import { z } from 'zod';

export const explorerSearchSchema = z.object({
  blueprint: z.string().trim().min(1).optional().catch(undefined),
  version: z.coerce.number().int().positive().optional().catch(undefined),
  query: z.string().trim().min(1).optional().catch(undefined),
});

export type ExplorerSearch = z.infer<typeof explorerSearchSchema>;

export const parseExplorerSearch = (
  input: Record<string, unknown>,
): ExplorerSearch => {
  return explorerSearchSchema.parse(input);
};

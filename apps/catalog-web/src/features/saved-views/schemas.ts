import { z } from 'zod';
import { explorerSearchSchema } from '../explorer/search';

export const savedViewSchema = z.object({
  id: z.uuid(),
  owner_user_id: z.uuid(),
  kind: z.literal('explorer_search'),
  name: z.string().nullable(),
  description: z.string().nullable(),
  visibility: z.enum(['private', 'workspace', 'link']),
  state: explorerSearchSchema,
  created_at: z.string(),
  updated_at: z.string(),
});
export type SavedView = z.infer<typeof savedViewSchema>;

import { z } from 'zod';
import { explorerSearchSchema } from '../explorer/search';
import {
  EXPLORER_SEARCH_KIND,
  SAVED_VIEW_VISIBILITY_LINK,
  SAVED_VIEW_VISIBILITY_PRIVATE,
  SAVED_VIEW_VISIBILITY_WORKSPACE,
} from './constants';

export const savedViewSchema = z.object({
  id: z.uuid(),
  owner_user_id: z.uuid(),
  kind: z.literal(EXPLORER_SEARCH_KIND),
  name: z.string().nullable(),
  description: z.string().nullable(),
  visibility: z.enum([
    SAVED_VIEW_VISIBILITY_PRIVATE,
    SAVED_VIEW_VISIBILITY_WORKSPACE,
    SAVED_VIEW_VISIBILITY_LINK,
  ]),
  state: explorerSearchSchema,
  created_at: z.string(),
  updated_at: z.string(),
});
export type SavedView = z.infer<typeof savedViewSchema>;

import { z } from 'zod';
import { maxCommentLength } from './constants';

export const commentDraftSchema = z.object({ body: z.string() });
export const commentInputSchema = z.object({
  body: z
    .string()
    .refine(
      (body) =>
        body.trim().length > 0 &&
        [...body].length <= maxCommentLength &&
        !body.includes('\0'),
    ),
});
export const commentSchema = z.object({
  id: z.uuid(),
  author_user_id: z.uuid(),
  author_display_name: z.string().nullable(),
  author_email: z.string(),
  body: z.string(),
  revision: z.number().int().positive(),
  created_at: z.string().datetime(),
  updated_at: z.string().datetime(),
});
export const commentPageSchema = z.object({
  items: z.array(commentSchema),
  has_more: z.boolean(),
});
export type EntityComment = z.infer<typeof commentSchema>;
export type CommentCursor = { before_time: string; before_id: string };

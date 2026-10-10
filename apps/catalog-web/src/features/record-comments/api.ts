import { request, requestNoContent } from '../../api/request';
import {
  commentCountSchema,
  commentInputSchema,
  commentPageSchema,
  type CommentCursor,
} from './schemas';

const commentsPath = (recordId: string) =>
  `/api/v1/records/${encodeURIComponent(recordId)}/comments`;

export const listComments = (recordId: string, cursor?: CommentCursor) => {
  const query = cursor ? `?${new URLSearchParams(cursor)}` : '';
  return request(`${commentsPath(recordId)}${query}`, commentPageSchema);
};

export const getCommentCount = (recordId: string, signal?: AbortSignal) =>
  request(`${commentsPath(recordId)}/count`, commentCountSchema, { signal });

export const createComment = (recordId: string, body: string) =>
  requestNoContent(commentsPath(recordId), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(commentInputSchema.parse({ body })),
  });

export const updateComment = (
  recordId: string,
  commentId: string,
  revision: number,
  body: string,
) =>
  requestNoContent(
    `${commentsPath(recordId)}/${encodeURIComponent(commentId)}`,
    {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ...commentInputSchema.parse({ body }), revision }),
    },
  );

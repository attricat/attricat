import { request, requestNoContent } from '../../api/request';
import {
  commentInputSchema,
  commentPageSchema,
  type CommentCursor,
} from './schemas';

const commentsPath = (entityId: string) =>
  `/api/v1/entities/${encodeURIComponent(entityId)}/comments`;

export const listComments = (entityId: string, cursor?: CommentCursor) => {
  const query = cursor ? `?${new URLSearchParams(cursor)}` : '';
  return request(`${commentsPath(entityId)}${query}`, commentPageSchema);
};

export const createComment = (entityId: string, body: string) =>
  requestNoContent(commentsPath(entityId), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(commentInputSchema.parse({ body })),
  });

export const updateComment = (
  entityId: string,
  commentId: string,
  revision: number,
  body: string,
) =>
  requestNoContent(
    `${commentsPath(entityId)}/${encodeURIComponent(commentId)}`,
    {
      method: 'PATCH',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ ...commentInputSchema.parse({ body }), revision }),
    },
  );

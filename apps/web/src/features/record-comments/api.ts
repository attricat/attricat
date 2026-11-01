import { request, requestNoContent, jsonRequest } from '../../api/request';
import {
  commentCountSchema,
  commentInputSchema,
  commentPageSchema,
} from './schemas';
import { keysetQuery, type KeysetCursor } from '../../api/keysetCursor';

const commentsPath = (recordId: string) =>
  `/api/v1/records/${encodeURIComponent(recordId)}/comments`;

export const listComments = (recordId: string, cursor?: KeysetCursor) =>
  request(`${commentsPath(recordId)}${keysetQuery(cursor)}`, commentPageSchema);

export const getCommentCount = (recordId: string, signal?: AbortSignal) =>
  request(`${commentsPath(recordId)}/count`, commentCountSchema, { signal });

export const createComment = (recordId: string, body: string) =>
  requestNoContent(
    commentsPath(recordId),
    jsonRequest('POST', commentInputSchema.parse({ body })),
  );

export const updateComment = (
  recordId: string,
  commentId: string,
  revision: number,
  body: string,
) =>
  requestNoContent(
    `${commentsPath(recordId)}/${encodeURIComponent(commentId)}`,
    jsonRequest('PATCH', { ...commentInputSchema.parse({ body }), revision }),
  );

export const deleteComment = (
  recordId: string,
  commentId: string,
  revision: number,
) =>
  requestNoContent(
    `${commentsPath(recordId)}/${encodeURIComponent(commentId)}?${new URLSearchParams({ revision: String(revision) })}`,
    { method: 'DELETE' },
  );

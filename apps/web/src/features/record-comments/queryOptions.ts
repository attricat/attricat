import { infiniteQueryOptions } from '@tanstack/react-query';
import { readAheadStaleTimeMs } from '../records/queryOptions';
import { listComments } from './api';
import { commentQueryKeys } from './queryKeys';
import type { CommentCursor } from './schemas';

/** A record's comments, newest page first, as seen by the signed-in user. */
export const recordCommentsOptions = (
  session: { workspace_id: string; user_id: string } | undefined,
  recordId: string,
) =>
  infiniteQueryOptions({
    queryKey: commentQueryKeys.record(
      session?.workspace_id,
      session?.user_id,
      recordId,
    ),
    queryFn: ({ pageParam }) => listComments(recordId, pageParam),
    initialPageParam: undefined as CommentCursor | undefined,
    getNextPageParam: (page) => {
      const last = page.items.at(-1);
      return page.has_more && last
        ? { before_time: last.created_at, before_id: last.id }
        : undefined;
    },
    staleTime: readAheadStaleTimeMs,
    enabled: Boolean(session),
  });

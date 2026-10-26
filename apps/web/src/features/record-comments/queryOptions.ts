import { infiniteQueryOptions } from '@tanstack/react-query';
import { readAheadStaleTimeMs } from '../records/queryOptions';
import { listComments } from './api';
import { commentQueryKeys } from './queryKeys';
import {
  createdAtCursor,
  nextKeysetCursor,
  type KeysetCursor,
} from '../../api/keysetCursor';

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
    initialPageParam: undefined as KeysetCursor | undefined,
    getNextPageParam: (page) => nextKeysetCursor(page, createdAtCursor),
    staleTime: readAheadStaleTimeMs,
    enabled: Boolean(session),
  });

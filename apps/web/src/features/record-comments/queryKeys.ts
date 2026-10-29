import type { QueryKey } from '@tanstack/react-query';

const commentsRoot = 'record-comments';

export const commentQueryKeys = {
  record: (
    workspaceId: string | undefined,
    userId: string | undefined,
    recordId: string,
  ) => [commentsRoot, workspaceId, userId, recordId] as const,
  /** Under `record`, so refreshing the comments refreshes the count. */
  count: (
    workspaceId: string | undefined,
    userId: string | undefined,
    recordId: string,
  ) =>
    [
      ...commentQueryKeys.record(workspaceId, userId, recordId),
      'count',
    ] as const,
};

/** Whether a query holds a record's comments, whoever reads them. */
export const isRecordCommentsQuery = (queryKey: QueryKey, recordId: string) => {
  const [root, , , record] = queryKey;
  return root === commentsRoot && record === recordId;
};

export const commentQueryKeys = {
  record: (
    workspaceId: string | undefined,
    userId: string | undefined,
    recordId: string,
  ) => ['record-comments', workspaceId, userId, recordId] as const,
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

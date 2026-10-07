export const commentQueryKeys = {
  entity: (
    workspaceId: string | undefined,
    userId: string | undefined,
    entityId: string,
  ) => ['entity-comments', workspaceId, userId, entityId] as const,
  /** Under `entity`, so refreshing the comments refreshes the count. */
  count: (
    workspaceId: string | undefined,
    userId: string | undefined,
    entityId: string,
  ) =>
    [
      ...commentQueryKeys.entity(workspaceId, userId, entityId),
      'count',
    ] as const,
};

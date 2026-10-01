export const commentQueryKeys = {
  entity: (
    workspaceId: string | undefined,
    userId: string | undefined,
    entityId: string,
  ) => ['entity-comments', workspaceId, userId, entityId] as const,
};

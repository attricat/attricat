export const backgroundProcessingQueryKeys = {
  status: (workspaceId: string | undefined) =>
    ['background-processing', workspaceId] as const,
};

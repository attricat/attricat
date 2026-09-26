export const savedViewQueryKeys = {
  all: () => ['saved-views'] as const,
  list: () => [...savedViewQueryKeys.all(), 'list'] as const,
  detail: (id: string, link: boolean) =>
    [...savedViewQueryKeys.all(), link ? 'link' : 'detail', id] as const,
};

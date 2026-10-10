export const savedViewQueryKeys = {
  all: () => ['saved-views'] as const,
  list: (search: string) =>
    [...savedViewQueryKeys.all(), 'list', search] as const,
  detail: (id: string, link: boolean) =>
    [...savedViewQueryKeys.all(), link ? 'link' : 'detail', id] as const,
};

import { queryOptions } from '@tanstack/react-query';
import { getSavedView } from './api';
import { savedViewQueryKeys } from './queryKeys';

/** A saved search, or with `link` a shared view-state link, by id. */
export const savedViewOptions = (id: string, link: boolean) =>
  queryOptions({
    queryKey: savedViewQueryKeys.detail(id, link),
    queryFn: ({ signal }) => getSavedView(id, link, signal),
  });

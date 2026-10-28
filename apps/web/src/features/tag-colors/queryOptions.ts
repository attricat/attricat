import { queryOptions } from '@tanstack/react-query';
import { getTagColors } from './api';
import { tagColorsStaleTimeMs } from './constants';
import { tagColorQueryKeys } from './queryKeys';

export const tagColorsOptions = () =>
  queryOptions({
    queryKey: tagColorQueryKeys.all(),
    queryFn: ({ signal }) => getTagColors(signal),
    staleTime: tagColorsStaleTimeMs,
  });

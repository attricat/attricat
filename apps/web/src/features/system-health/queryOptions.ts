import { queryOptions } from '@tanstack/react-query';
import { getSystemHealth } from './api';
import { systemHealthQueryKeys } from './queryKeys';

/** The server's health report. */
export const systemHealthOptions = () =>
  queryOptions({
    queryKey: systemHealthQueryKeys.health(),
    queryFn: getSystemHealth,
  });

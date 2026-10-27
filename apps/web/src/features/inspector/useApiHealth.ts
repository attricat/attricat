import { useQuery } from '@tanstack/react-query';
import { getApiHealth } from './api';
import { type ApiHealthState, apiHealthStates } from './apiHealth';
import {
  API_HEALTH_POLL_INTERVAL_MS,
  API_RETRY_INTERVAL_MS,
} from './constants';
import { inspectorEnabled } from './enabled';
import { inspectorQueryKeys } from './queryKeys';

/**
 * Polls the API's health while the Inspector is enabled, retrying quickly
 * while it is unreachable. Every caller shares one query, so the Inspector and
 * the navigation mark poll once between them. Without the Inspector nothing
 * polls and the state stays `checking`.
 */
export const useApiHealth = (): ApiHealthState => {
  const query = useQuery({
    queryKey: inspectorQueryKeys.apiHealth(),
    queryFn: getApiHealth,
    enabled: inspectorEnabled,
    retry: false,
    refetchInterval: (query) =>
      query.state.status === 'error'
        ? API_RETRY_INTERVAL_MS
        : API_HEALTH_POLL_INTERVAL_MS,
  });
  if (query.isError) return apiHealthStates.restarting;
  if (query.isPending) return apiHealthStates.checking;
  return apiHealthStates.ready;
};

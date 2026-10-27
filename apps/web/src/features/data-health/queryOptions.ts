import { queryOptions } from '@tanstack/react-query';
import {
  getDataHealthBlueprints,
  getDataHealthCompleteness,
  getDataHealthContexts,
  getDataHealthFreshness,
  getDataHealthRelationships,
  getDataHealthStorage,
  getDataHealthSummary,
} from './api';
import { dataHealthStaleTime } from './constants';
import { dataHealthQueryKeys } from './queryKeys';

/** The Data Health reports; each stays fresh for `dataHealthStaleTime`. */
export const dataHealthOptions = {
  summary: (staleAfterDays: number) =>
    queryOptions({
      queryKey: dataHealthQueryKeys.summary(staleAfterDays),
      queryFn: () => getDataHealthSummary(staleAfterDays),
      staleTime: dataHealthStaleTime,
    }),
  blueprints: (staleAfterDays: number) =>
    queryOptions({
      queryKey: dataHealthQueryKeys.blueprints(staleAfterDays),
      queryFn: () => getDataHealthBlueprints(staleAfterDays),
      staleTime: dataHealthStaleTime,
    }),
  freshness: () =>
    queryOptions({
      queryKey: dataHealthQueryKeys.freshness(),
      queryFn: getDataHealthFreshness,
      staleTime: dataHealthStaleTime,
    }),
  storage: () =>
    queryOptions({
      queryKey: dataHealthQueryKeys.storage(),
      queryFn: getDataHealthStorage,
      staleTime: dataHealthStaleTime,
    }),
  completeness: () =>
    queryOptions({
      queryKey: dataHealthQueryKeys.completeness(),
      queryFn: getDataHealthCompleteness,
      staleTime: dataHealthStaleTime,
    }),
  contexts: () =>
    queryOptions({
      queryKey: dataHealthQueryKeys.contexts(),
      queryFn: getDataHealthContexts,
      staleTime: dataHealthStaleTime,
    }),
  relationships: () =>
    queryOptions({
      queryKey: dataHealthQueryKeys.relationships(),
      queryFn: getDataHealthRelationships,
      staleTime: dataHealthStaleTime,
    }),
};

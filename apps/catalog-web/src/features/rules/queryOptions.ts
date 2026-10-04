import { queryOptions } from '@tanstack/react-query';
import { listRules } from './api';
import { ruleQueryKeys } from './queryKeys';

export const ruleDefinitionsOptions = () =>
  queryOptions({
    queryKey: ruleQueryKeys.definitions(),
    queryFn: listRules,
  });

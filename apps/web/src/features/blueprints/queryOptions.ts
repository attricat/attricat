import { queryOptions } from '@tanstack/react-query';
import { listBlueprints } from './api';
import { blueprintQueryKeys } from './queryKeys';

/** The workspace's blueprints, latest revision of each. */
export const blueprintCatalogueOptions = () =>
  queryOptions({
    queryKey: blueprintQueryKeys.catalogue(),
    queryFn: listBlueprints,
  });

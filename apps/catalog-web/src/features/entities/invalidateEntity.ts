import type { QueryClient } from '@tanstack/react-query';
import { entityQueryKeys } from './queryKeys';

export const invalidateEntitySearches = (client: QueryClient) =>
  client.invalidateQueries({ queryKey: entityQueryKeys.searches() });

/** Publication changes also affect server-side search ordering. */
export const invalidateEntityPublications = (client: QueryClient, id: string) =>
  Promise.all([
    client.invalidateQueries({ queryKey: entityQueryKeys.publication(id) }),
    invalidateEntitySearches(client),
  ]);

const entityProjections = (id: string) => [
  entityQueryKeys.form(id),
  entityQueryKeys.preview(id),
  entityQueryKeys.resolvedPreviews(id),
  entityQueryKeys.changes(id),
  entityQueryKeys.recordControls(id),
  entityQueryKeys.publication(id),
  entityQueryKeys.migrationPreview(id),
  entityQueryKeys.hierarchies(id),
  entityQueryKeys.incomingRelationshipResults(id),
];

/** Every projection of an entity must be refreshed after its values change. */
export const invalidateEntity = (client: QueryClient, id: string) =>
  Promise.all([
    ...entityProjections(id).map((queryKey) =>
      client.invalidateQueries({ queryKey }),
    ),
    invalidateEntitySearches(client),
  ]);

/** Do not refetch a deleted entity; discard its cached projections instead. */
export const removeEntity = (client: QueryClient, id: string) => {
  for (const queryKey of entityProjections(id)) {
    client.removeQueries({ queryKey });
  }
  return invalidateEntitySearches(client);
};

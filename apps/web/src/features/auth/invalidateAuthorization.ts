import type { QueryClient } from '@tanstack/react-query';
import { authQueryKeys } from './queryKeys';

/**
 * Refreshes what the caller may do after roles or grants change: the
 * session's workspace-wide capabilities and every scoped access check.
 */
export const invalidateAuthorization = (client: QueryClient) =>
  Promise.all([
    client.invalidateQueries({ queryKey: authQueryKeys.session() }),
    client.invalidateQueries({ queryKey: authQueryKeys.accessAll() }),
  ]);

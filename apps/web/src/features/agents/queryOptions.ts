import {
  infiniteQueryOptions,
  queryOptions,
  type QueryClient,
  skipToken,
} from '@tanstack/react-query';
import {
  getConversation,
  listApprovals,
  listMessages,
  listRuns,
  searchConversations,
  type ConversationFilter,
} from './api';
import { titlePollInterval } from './titlePolling';
import {
  activeRunStatuses,
  conversationScopes,
  conversationIdlePollIntervalMs,
  conversationPollIntervalMs,
  conversationReconcileIntervalMs,
} from './constants';
import { agentQueryKeys } from './queryKeys';
import type { AgentRun } from './schemas';

/** A conversation; waits while none is chosen. */
export const conversationOptions = (id: string | null) =>
  queryOptions({
    queryKey: agentQueryKeys.conversation(id ?? ''),
    queryFn: id ? ({ signal }) => getConversation(id, signal) : skipToken,
  });
export const conversationMessagesOptions = (id: string) =>
  queryOptions({
    queryKey: agentQueryKeys.messages(id),
    queryFn: ({ signal }) => listMessages(id, signal),
    staleTime: conversationPollIntervalMs,
  });
export const conversationRunsOptions = (id: string) =>
  queryOptions({
    queryKey: agentQueryKeys.runs(id),
    queryFn: ({ signal }) => listRuns(id, signal),
    staleTime: conversationPollIntervalMs,
  });
export const conversationApprovalsOptions = (id: string) =>
  queryOptions({
    queryKey: agentQueryKeys.approvals(id),
    queryFn: ({ signal }) => listApprovals(id, signal),
    staleTime: conversationPollIntervalMs,
  });

export const conversationPollInterval = (
  runs: AgentRun[] | undefined,
  connected: boolean,
  sending = false,
) => {
  if (
    !sending &&
    runs &&
    !runs.some((run) => activeRunStatuses.includes(run.status))
  )
    return conversationIdlePollIntervalMs;
  return connected
    ? conversationReconcileIntervalMs
    : conversationPollIntervalMs;
};

export const invalidateConversation = (client: QueryClient, id: string) =>
  Promise.all([
    client.invalidateQueries({ queryKey: agentQueryKeys.conversation(id) }),
    client.invalidateQueries({ queryKey: agentQueryKeys.approvals(id) }),
  ]);

/**
 * A page-by-page conversation search, either about one record or by scope.
 * It polls while a listed conversation is still waiting for its title.
 */
export const conversationSearchOptions = (
  query: string,
  filter: ConversationFilter,
) =>
  infiniteQueryOptions({
    queryKey: filter.recordId
      ? agentQueryKeys.recordConversations(filter.recordId)
      : agentQueryKeys.conversationSearch(
          query,
          filter.scope ?? conversationScopes.unattached,
        ),
    queryFn: ({ pageParam }) => searchConversations(query, pageParam, filter),
    initialPageParam: undefined as string | undefined,
    getNextPageParam: (page) => page.next_cursor ?? undefined,
    refetchInterval: (search) =>
      titlePollInterval(search.state.data?.pages.flatMap((page) => page.items)),
  });

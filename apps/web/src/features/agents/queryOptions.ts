import { queryOptions, type QueryClient } from '@tanstack/react-query';
import { getConversation, listApprovals, listMessages, listRuns } from './api';
import {
  activeRunStatuses,
  conversationIdlePollIntervalMs,
  conversationPollIntervalMs,
  conversationReconcileIntervalMs,
} from './constants';
import { agentQueryKeys } from './queryKeys';
import type { AgentRun } from './schemas';

export const conversationOptions = (id: string) =>
  queryOptions({
    queryKey: agentQueryKeys.conversation(id),
    queryFn: ({ signal }) => getConversation(id, signal),
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

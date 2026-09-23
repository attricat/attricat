import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { agentRunEventsUrl } from './api';
import { agentQueryKeys } from './query-keys';
import type { AgentRun } from './schemas';

const activeRunStatuses = ['queued', 'running', 'awaiting_approval'];
const updateEvents = [
  'status',
  'message',
  'tool_call',
  'approval_required',
  'error',
  'terminal',
];

export const useConversationLiveUpdates = (runs: AgentRun[] | undefined) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [streamError, setStreamError] = useState<string | null>(null);
  // Polling produces new run arrays even when the active subscriptions have
  // not changed. Depend on their IDs instead of reconnecting on every poll.
  const activeRunIds = (runs ?? [])
    .filter((run) => activeRunStatuses.includes(run.status))
    .map((run) => run.id)
    .sort()
    .join(',');

  useEffect(() => {
    const sources = activeRunIds
      ? activeRunIds.split(',').map((id) => {
          const source = new EventSource(agentRunEventsUrl(id));
          const update = () => {
            setStreamError(null);
            void queryClient.invalidateQueries({
              queryKey: agentQueryKeys.all(),
            });
          };

          updateEvents.forEach((type) => source.addEventListener(type, update));
          source.onerror = () =>
            setStreamError(t('agents.liveUpdatesDisconnected'));
          return source;
        })
      : [];

    return () => sources.forEach((source) => source.close());
  }, [activeRunIds, queryClient, t]);

  return activeRunIds ? streamError : null;
};

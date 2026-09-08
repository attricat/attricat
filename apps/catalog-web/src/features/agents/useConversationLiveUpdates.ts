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
  'schedule_skipped',
];

export const useConversationLiveUpdates = (runs: AgentRun[] | undefined) => {
  const { t } = useTranslation();
  const queryClient = useQueryClient();
  const [streamError, setStreamError] = useState<string | null>(null);

  useEffect(() => {
    const sources =
      runs
        ?.filter((run) => activeRunStatuses.includes(run.status))
        .map((run) => {
          const source = new EventSource(agentRunEventsUrl(run.id));
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
        }) ?? [];

    return () => sources.forEach((source) => source.close());
  }, [queryClient, runs, t]);

  return streamError;
};

import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useQueryClient } from '@tanstack/react-query';
import { agentRunEventsUrl } from './api';
import {
  activeRunStatuses,
  conversationEventBatchDelayMs,
  runUpdateEvents,
} from './constants';
import { invalidateConversation } from './queryOptions';
import type { AgentRun } from './schemas';

export const useConversationLiveUpdates = (
  conversationId: string,
  runs: AgentRun[] | undefined,
) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const activeRunIds = (runs ?? [])
    .filter((run) => activeRunStatuses.includes(run.status))
    .map((run) => run.id)
    .sort()
    .join(',');
  const subscriptionKey = `${conversationId}:${activeRunIds}`;
  const [connection, setConnection] = useState<{
    key: string;
    connected: boolean;
    failed: boolean;
  }>();

  useEffect(() => {
    if (!activeRunIds || typeof EventSource === 'undefined') return;
    const ids = activeRunIds.split(',');
    const open = new Set<string>();
    let disposed = false;
    let refreshTimer: ReturnType<typeof setTimeout> | undefined;
    const report = (failed: boolean) => {
      if (!disposed)
        setConnection({
          key: subscriptionKey,
          connected: open.size === ids.length,
          failed,
        });
    };
    const scheduleRefresh = () => {
      if (disposed || refreshTimer !== undefined) return;
      refreshTimer = setTimeout(() => {
        refreshTimer = undefined;
        void invalidateConversation(client, conversationId);
      }, conversationEventBatchDelayMs);
    };
    const sources = ids.map((id) => {
      const source = new EventSource(agentRunEventsUrl(id));
      source.onopen = () => {
        open.add(id);
        report(false);
        // Reconcile anything missed while disconnected.
        scheduleRefresh();
      };
      runUpdateEvents.forEach((type) =>
        source.addEventListener(type, scheduleRefresh),
      );
      source.onerror = () => {
        open.delete(id);
        report(true);
      };
      return source;
    });
    return () => {
      disposed = true;
      clearTimeout(refreshTimer);
      sources.forEach((source) => source.close());
    };
  }, [activeRunIds, client, conversationId, subscriptionKey]);

  const current = connection?.key === subscriptionKey ? connection : undefined;
  return {
    connected: Boolean(activeRunIds && current?.connected),
    error:
      activeRunIds && current?.failed
        ? t('agents.liveUpdatesDisconnected')
        : null,
  };
};

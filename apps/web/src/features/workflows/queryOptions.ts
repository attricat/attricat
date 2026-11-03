import {
  infiniteQueryOptions,
  queryOptions,
  skipToken,
} from '@tanstack/react-query';
import { activeRunStatuses } from '../../api/runStatus';
import {
  getWorkflowRevision,
  listWorkflowRevisions,
  listWorkflowRunSummaries,
  listWorkflowRuns,
  listWorkflows,
} from './api';
import { workflowRunPollMs } from './constants';
import { workflowQueryKeys } from './queryKeys';

export const workflowListOptions = () =>
  queryOptions({ queryKey: workflowQueryKeys.all(), queryFn: listWorkflows });

/** One workflow's runs, refreshed while one has not finished. */
export const workflowRunListOptions = (workflowId: string) =>
  infiniteQueryOptions({
    queryKey: workflowQueryKeys.workflowRuns(workflowId),
    queryFn: ({ pageParam }) => listWorkflowRuns(workflowId, pageParam),
    initialPageParam: undefined as number | undefined,
    getNextPageParam: (page) => page.nextOffset,
    refetchInterval: (query) =>
      query.state.data?.pages.some((page) =>
        page.items.some((run) => activeRunStatuses.includes(run.status)),
      )
        ? workflowRunPollMs
        : false,
  });

/** What the list shows about each workflow's runs. */
export const workflowRunSummaryOptions = () =>
  queryOptions({
    queryKey: workflowQueryKeys.runSummaries(),
    queryFn: listWorkflowRunSummaries,
    refetchInterval: (query) =>
      query.state.data?.some((summary) =>
        activeRunStatuses.includes(summary.latest_status),
      )
        ? workflowRunPollMs
        : false,
  });

export const workflowRevisionListOptions = (workflowId: string) =>
  queryOptions({
    queryKey: workflowQueryKeys.revisions(workflowId),
    queryFn: () => listWorkflowRevisions(workflowId),
  });

/** One revision of a workflow; waits while either is unknown. */
export const workflowRevisionOptions = (
  workflowId: string | undefined,
  version: number | undefined,
) =>
  queryOptions({
    queryKey: workflowQueryKeys.revision(workflowId ?? '', version ?? 0),
    queryFn:
      workflowId && version
        ? () => getWorkflowRevision(workflowId, version)
        : skipToken,
  });

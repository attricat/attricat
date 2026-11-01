import { queryOptions, skipToken } from '@tanstack/react-query';
import { activeRunStatuses } from '../../api/runStatus';
import {
  getWorkflowRevision,
  listWorkflowRevisions,
  listWorkflowRuns,
  listWorkflows,
} from './api';
import { workflowRunPollMs } from './constants';
import { workflowQueryKeys } from './queryKeys';

export const workflowListOptions = () =>
  queryOptions({ queryKey: workflowQueryKeys.all(), queryFn: listWorkflows });

/** The workspace's workflow runs, refreshed while one has not finished. */
export const workflowRunListOptions = () =>
  queryOptions({
    queryKey: workflowQueryKeys.runs(),
    queryFn: listWorkflowRuns,
    refetchInterval: (query) =>
      query.state.data?.some((run) => activeRunStatuses.includes(run.status))
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

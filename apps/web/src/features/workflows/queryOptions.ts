import { queryOptions, skipToken } from '@tanstack/react-query';
import {
  getWorkflowRevision,
  listWorkflowRevisions,
  listWorkflowRuns,
  listWorkflows,
} from './api';
import { workflowQueryKeys } from './queryKeys';

export const workflowListOptions = () =>
  queryOptions({ queryKey: workflowQueryKeys.all(), queryFn: listWorkflows });

export const workflowRunListOptions = () =>
  queryOptions({
    queryKey: workflowQueryKeys.runs(),
    queryFn: listWorkflowRuns,
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

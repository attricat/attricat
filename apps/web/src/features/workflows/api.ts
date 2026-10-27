import { z } from 'zod';
import { request, requestNoContent, jsonRequest } from '../../api/request';
import {
  compiledWorkflowSchema,
  workflowRunSchema,
  workflowSchema,
  type CompiledWorkflow,
  type Workflow,
  type WorkflowRun,
} from './schemas';
import { uuidSegment, versionSegment } from '../../api/pathSegments';
import { maximumRunIdempotencyKeyLength } from './constants';

export type { Workflow } from './schemas';

const definitionRequest = (definition: string) =>
  jsonRequest('POST', { definition });

export const listWorkflows = (): Promise<Workflow[]> =>
  request('/api/workflows', z.array(workflowSchema));

export const listWorkflowRevisions = (id: string): Promise<Workflow[]> =>
  request(
    `/api/workflows/${uuidSegment(id)}/versions`,
    z.array(workflowSchema),
  );

export const getWorkflowRevision = (
  id: string,
  version: number,
): Promise<Workflow> =>
  request(
    `/api/workflows/${uuidSegment(id)}/versions/${versionSegment(version)}`,
    workflowSchema,
  );

export const validateWorkflow = (
  definition: string,
): Promise<CompiledWorkflow> =>
  request(
    '/api/workflows/validate',
    compiledWorkflowSchema,
    definitionRequest(definition),
  );

export const createWorkflow = (definition: string): Promise<Workflow> =>
  request('/api/workflows', workflowSchema, definitionRequest(definition));

export const createWorkflowRevision = (
  id: string,
  definition: string,
): Promise<Workflow> =>
  request(
    `/api/workflows/${uuidSegment(id)}/versions`,
    workflowSchema,
    definitionRequest(definition),
  );

export const publishWorkflowRevision = (
  id: string,
  version: number,
): Promise<Workflow> =>
  request(
    `/api/workflows/${uuidSegment(id)}/versions/${versionSegment(version)}/publish`,
    workflowSchema,
    { method: 'POST' },
  );

export const enableWorkflowRevision = (
  id: string,
  version: number,
): Promise<Workflow> =>
  request(
    `/api/workflows/${uuidSegment(id)}/versions/${versionSegment(version)}/enable`,
    workflowSchema,
    { method: 'POST' },
  );

export const runWorkflowNow = (
  id: string,
  recordId: string,
  idempotencyKey: string,
): Promise<{ id: string }> =>
  request(
    `/api/workflows/${uuidSegment(id)}/run-now`,
    z.object({ id: z.uuid() }),
    jsonRequest('POST', {
      record_id: z.uuid().parse(recordId),
      idempotency_key: z
        .string()
        .min(1)
        .max(maximumRunIdempotencyKeyLength)
        .parse(idempotencyKey),
    }),
  );

export const disableWorkflow = (id: string): Promise<Workflow> =>
  request(`/api/workflows/${uuidSegment(id)}/disable`, workflowSchema, {
    method: 'POST',
  });

export const listWorkflowRuns = (): Promise<WorkflowRun[]> =>
  request('/api/workflow-runs', z.array(workflowRunSchema));

export const replayWorkflowRun = (id: string) =>
  requestNoContent(`/api/workflow-runs/${uuidSegment(id)}/replay`, {
    method: 'POST',
  });

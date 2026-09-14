import { z } from 'zod';
import { request, requestNoContent } from '../../api/request';
import {
  compiledWorkflowSchema,
  workflowRunSchema,
  workflowSchema,
  type CompiledWorkflow,
  type Workflow,
  type WorkflowRun,
} from './schemas';

export type { CompiledWorkflow, Workflow, WorkflowRun } from './schemas';

const workflowPath = (id: string) => encodeURIComponent(z.uuid().parse(id));
const versionPath = (version: number) =>
  z.number().int().positive().parse(version);
const definitionRequest = (definition: string) => ({
  body: JSON.stringify({ definition }),
  headers: { 'Content-Type': 'application/json' },
  method: 'POST',
});

export const listWorkflows = (): Promise<Workflow[]> =>
  request('/api/workflows', z.array(workflowSchema));

export const getWorkflow = (id: string): Promise<Workflow> =>
  request(`/api/workflows/${workflowPath(id)}`, workflowSchema);

export const listWorkflowRevisions = (id: string): Promise<Workflow[]> =>
  request(
    `/api/workflows/${workflowPath(id)}/versions`,
    z.array(workflowSchema),
  );

export const getWorkflowRevision = (
  id: string,
  version: number,
): Promise<Workflow> =>
  request(
    `/api/workflows/${workflowPath(id)}/versions/${versionPath(version)}`,
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
    `/api/workflows/${workflowPath(id)}/versions`,
    workflowSchema,
    definitionRequest(definition),
  );

export const publishWorkflowRevision = (
  id: string,
  version: number,
): Promise<Workflow> =>
  request(
    `/api/workflows/${workflowPath(id)}/versions/${versionPath(version)}/publish`,
    workflowSchema,
    { method: 'POST' },
  );

export const enableWorkflowRevision = (
  id: string,
  version: number,
): Promise<Workflow> =>
  request(
    `/api/workflows/${workflowPath(id)}/versions/${versionPath(version)}/enable`,
    workflowSchema,
    { method: 'POST' },
  );

export const runWorkflowNow = (
  id: string,
  entityId: string,
  idempotencyKey: string,
): Promise<{ id: string }> =>
  request(
    `/api/workflows/${workflowPath(id)}/run-now`,
    z.object({ id: z.uuid() }),
    {
      body: JSON.stringify({
        entity_id: z.uuid().parse(entityId),
        idempotency_key: z.string().min(1).max(128).parse(idempotencyKey),
      }),
      headers: { 'Content-Type': 'application/json' },
      method: 'POST',
    },
  );

export const disableWorkflow = (id: string): Promise<Workflow> =>
  request(`/api/workflows/${workflowPath(id)}/disable`, workflowSchema, {
    method: 'POST',
  });

export const listWorkflowRuns = (): Promise<WorkflowRun[]> =>
  request('/api/workflow-runs', z.array(workflowRunSchema));

export const replayWorkflowRun = (id: string) =>
  requestNoContent(`/api/workflow-runs/${workflowPath(id)}/replay`, {
    method: 'POST',
  });

import { z } from 'zod';
import { revisionStatusSchema } from '../../api/revisionStatus';
import { runStatusSchema } from '../../api/runStatus';
import { workflowRunSources } from './constants';

export const workflowSchema = z.object({
  id: z.uuid(),
  code: z.string(),
  name: z.string(),
  version: z.number().int().positive(),
  status: revisionStatusSchema,
  definition: z.string(),
  definition_hash: z.string(),
  compiled_plan: z.unknown(),
  published_at: z.string().nullable(),
  created_at: z.string(),
  enabled_version: z.number().int().positive().nullable(),
  manual_enabled: z.boolean(),
});

export const workflowRunSchema = z.object({
  id: z.uuid(),
  workflow_id: z.uuid(),
  workflow_version: z.number().int().positive(),
  trigger_event_id: z.uuid().nullable(),
  trigger_sequence: z.number().int().nullable(),
  source: z.enum(workflowRunSources).default(workflowRunSources.event),
  status: runStatusSchema,
  attempts: z.number().int().nonnegative(),
  failed_at: z.string().nullable(),
  completed_at: z.string().nullable(),
  last_error: z.string().nullable(),
  created_at: z.string(),
  cancelled_at: z.string().nullable(),
  root_trigger_event_id: z.uuid().nullable(),
  causal_depth: z.number().int().nonnegative(),
});

export const compiledWorkflowSchema = z.object({
  code: z.string(),
  name: z.string(),
  format_version: z.number().int(),
  triggers: z.array(z.unknown()),
  actions: z.array(z.unknown()),
  raw_definition_hash: z.string(),
});

export type Workflow = z.infer<typeof workflowSchema>;
export type WorkflowRun = z.infer<typeof workflowRunSchema>;
export type CompiledWorkflow = z.infer<typeof compiledWorkflowSchema>;

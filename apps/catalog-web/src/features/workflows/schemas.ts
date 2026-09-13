import { z } from 'zod';

export const workflowSchema = z.object({
  id: z.uuid(),
  code: z.string(),
  name: z.string(),
  version: z.number().int().positive(),
  status: z.enum(['draft', 'published']),
  definition: z.string(),
  definition_hash: z.string(),
  compiled_plan: z.unknown(),
  published_at: z.string().nullable(),
  created_at: z.string(),
  enabled_version: z.number().int().positive().nullable(),
});

export const workflowRunSchema = z.object({
  id: z.uuid(),
  workflow_id: z.uuid(),
  workflow_version: z.number().int().positive(),
  trigger_event_id: z.uuid().nullable(),
  trigger_sequence: z.number().int().nullable(),
  source: z.enum(['event', 'manual', 'schedule']).default('event'),
  status: z.enum([
    'pending',
    'leased',
    'completed',
    'dead_letter',
    'cancelled',
  ]),
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

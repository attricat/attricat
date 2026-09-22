import { z } from 'zod';
import { request } from '../../api/request';
import { findingSchema, ruleRunSchema, ruleSchema } from './schemas';
export type { Finding, Rule, RuleRun } from './schemas';
const id = (value: string) => encodeURIComponent(z.uuid().parse(value));
export const listRules = () => request('/api/rules', z.array(ruleSchema));
export const createRule = (input: {
  blueprintId: string;
  blueprintVersion: number;
  contextId?: string;
  definition: string;
}) =>
  request('/api/rules', ruleSchema, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      blueprint_id: z.uuid().parse(input.blueprintId),
      blueprint_version: z
        .number()
        .int()
        .positive()
        .parse(input.blueprintVersion),
      context_id: input.contextId ? z.uuid().parse(input.contextId) : null,
      definition: input.definition,
    }),
  });
export const listRuleRuns = () =>
  request('/api/rule-runs', z.array(ruleRunSchema));
export const listFindings = (entityId?: string) =>
  request(
    entityId
      ? `/api/rule-findings?entity_id=${id(entityId)}`
      : '/api/rule-findings',
    z.array(findingSchema),
  );
export const acknowledgeFinding = (findingId: string) =>
  request(`/api/rule-findings/${id(findingId)}/acknowledge`, findingSchema, {
    method: 'POST',
  });
export const runRuleNow = (ruleId: string, dryRun: boolean) =>
  request(`/api/rules/${id(ruleId)}/run-now`, z.object({ id: z.uuid() }), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      dry_run: dryRun,
      entity_id: null,
      idempotency_key: crypto.randomUUID(),
    }),
  });

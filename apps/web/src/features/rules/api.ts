import { z } from 'zod';
import { request, requestOffsetPage } from '../../api/request';
import { RECORD_FINDINGS_PAGE_SIZE, RECORD_RULE_RUNS_LIMIT } from './constants';
import { findingSchema, ruleRunSchema, ruleSchema } from './schemas';
export type { Finding, Rule, RuleRun } from './schemas';
const id = (value: string) => encodeURIComponent(z.uuid().parse(value));
export const listRules = () => request('/api/rules', z.array(ruleSchema));
export const listRuleRuns = () =>
  request('/api/rule-runs', z.array(ruleRunSchema));
/** The most recent runs scoped to one record. */
export const listRecordRuleRuns = (recordId: string) =>
  request(
    `/api/rule-runs?record_id=${id(recordId)}&limit=${RECORD_RULE_RUNS_LIMIT}`,
    z.array(ruleRunSchema),
  );
/** Re-checks one record against every enabled rule of its blueprint revision. */
export const recheckRecord = (recordId: string) =>
  request(
    '/api/rule-findings/recheck',
    z.object({ run_ids: z.array(z.uuid()) }),
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        record_id: z.uuid().parse(recordId),
        idempotency_key: crypto.randomUUID(),
      }),
    },
  );
export const listFindings = (recordId?: string) =>
  request(
    recordId
      ? `/api/rule-findings?record_id=${id(recordId)}`
      : '/api/rule-findings',
    z.array(findingSchema),
  );
/** One page of a record's open and acknowledged findings, newest change first. */
export const listUnresolvedRecordFindings = (recordId: string, offset = 0) =>
  requestOffsetPage(
    `/api/rule-findings?${new URLSearchParams({
      record_id: z.uuid().parse(recordId),
      unresolved: 'true',
      limit: String(RECORD_FINDINGS_PAGE_SIZE),
      offset: String(offset),
    })}`,
    z.array(findingSchema),
  );
/** How many of a record's findings are open or acknowledged. */
export const getUnresolvedRecordFindingCount = (recordId: string) =>
  request(
    `/api/rule-findings/count?record_id=${id(recordId)}`,
    z.object({ count: z.number().int().nonnegative() }),
  );
export const acknowledgeFinding = (findingId: string) =>
  request(`/api/rule-findings/${id(findingId)}/acknowledge`, findingSchema, {
    method: 'POST',
  });
export const runRuleNow = (ruleId: string, version: number, dryRun: boolean) =>
  request(`/api/rules/${id(ruleId)}/run-now`, z.object({ id: z.uuid() }), {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      dry_run: dryRun,
      record_id: null,
      idempotency_key: crypto.randomUUID(),
      version: z.number().int().positive().parse(version),
    }),
  });
export const enableRuleRevision = (
  ruleId: string,
  version: number,
  acceptExistingViolations = false,
) =>
  request(
    `/api/rules/${id(ruleId)}/versions/${z.number().int().positive().parse(version)}/enable`,
    ruleSchema,
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(
        acceptExistingViolations ? { accept_existing_violations: true } : {},
      ),
    },
  );
export const disableRule = (ruleId: string) =>
  request(`/api/rules/${id(ruleId)}/disable`, ruleSchema, { method: 'POST' });

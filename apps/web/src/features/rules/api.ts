import { z } from 'zod';
import { request, requestOffsetPage, jsonRequest } from '../../api/request';
import { RECORD_FINDINGS_PAGE_SIZE, RECORD_RULE_RUNS_LIMIT } from './constants';
import { findingSchema, ruleRunSchema, ruleSchema } from './schemas';
import { uuidSegment, versionSegment } from '../../api/pathSegments';
import { randomUuid } from '../../components/randomUuid';
export type { Finding, Rule, RuleRun } from './schemas';
export const listRules = () => request('/api/rules', z.array(ruleSchema));
export const listRuleRuns = () =>
  request('/api/rule-runs', z.array(ruleRunSchema));
/** The most recent runs scoped to one record. */
export const listRecordRuleRuns = (recordId: string) =>
  request(
    `/api/rule-runs?record_id=${uuidSegment(recordId)}&limit=${RECORD_RULE_RUNS_LIMIT}`,
    z.array(ruleRunSchema),
  );
/** Re-checks one record against every enabled rule of its blueprint revision. */
export const recheckRecord = (recordId: string) =>
  request(
    '/api/rule-findings/recheck',
    z.object({ run_ids: z.array(z.uuid()) }),
    jsonRequest('POST', {
      record_id: z.uuid().parse(recordId),
      idempotency_key: randomUuid(),
    }),
  );
export const listFindings = (recordId?: string) =>
  request(
    recordId
      ? `/api/rule-findings?record_id=${uuidSegment(recordId)}`
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
    `/api/rule-findings/count?record_id=${uuidSegment(recordId)}`,
    z.object({ count: z.number().int().nonnegative() }),
  );
export const acknowledgeFinding = (findingId: string) =>
  request(
    `/api/rule-findings/${uuidSegment(findingId)}/acknowledge`,
    findingSchema,
    {
      method: 'POST',
    },
  );
export const runRuleNow = (ruleId: string, version: number, dryRun: boolean) =>
  request(
    `/api/rules/${uuidSegment(ruleId)}/run-now`,
    z.object({ id: z.uuid() }),
    jsonRequest('POST', {
      dry_run: dryRun,
      record_id: null,
      idempotency_key: randomUuid(),
      version: z.number().int().positive().parse(version),
    }),
  );
export const enableRuleRevision = (
  ruleId: string,
  version: number,
  acceptExistingViolations = false,
) =>
  request(
    `/api/rules/${uuidSegment(ruleId)}/versions/${versionSegment(version)}/enable`,
    ruleSchema,
    jsonRequest(
      'POST',
      acceptExistingViolations ? { accept_existing_violations: true } : {},
    ),
  );
export const disableRule = (ruleId: string) =>
  request(`/api/rules/${uuidSegment(ruleId)}/disable`, ruleSchema, {
    method: 'POST',
  });

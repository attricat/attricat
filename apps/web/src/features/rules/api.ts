import { z } from 'zod';
import { request, requestOffsetPage, jsonRequest } from '../../api/request';
import {
  FINDINGS_PAGE_SIZE,
  findingStateFilters,
  RECORD_FINDINGS_PAGE_SIZE,
  RECORD_RULE_RUNS_LIMIT,
} from './constants';
import {
  compiledRuleSchema,
  createRuleSchema,
  findingSchema,
  ruleRunSchema,
  ruleSchema,
  type CreateRule,
  type FindingsSearch,
} from './schemas';
import { uuidSegment, versionSegment } from '../../api/pathSegments';
import { randomUuid } from '../../components/randomUuid';
export type {
  CompiledRule,
  CreateRule,
  Finding,
  FindingsSearch,
  Rule,
  RuleRun,
} from './schemas';
export const listRules = () => request('/api/rules', z.array(ruleSchema));
/** Runs the save checks on a rule and its blueprint revision without storing it. */
export const validateRule = (input: CreateRule, signal?: AbortSignal) =>
  request('/api/rules/validate', compiledRuleSchema, {
    ...jsonRequest('POST', createRuleSchema.parse(input)),
    signal,
  });
/** Creates a rule as a draft first revision. */
export const createRule = (input: CreateRule) =>
  request(
    '/api/rules',
    ruleSchema,
    jsonRequest('POST', createRuleSchema.parse(input)),
  );
/** Adds a draft revision; the blueprint revision and context stay the same. */
export const createRuleRevision = (ruleId: string, input: CreateRule) =>
  request(
    `/api/rules/${uuidSegment(ruleId)}/versions`,
    ruleSchema,
    jsonRequest('POST', createRuleSchema.parse(input)),
  );
export const publishRuleRevision = (ruleId: string, version: number) =>
  request(
    `/api/rules/${uuidSegment(ruleId)}/versions/${versionSegment(version)}/publish`,
    ruleSchema,
    { method: 'POST' },
  );
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
/** One page of the workspace's findings that match the findings page's filters. */
export const listFindingsPage = (search: FindingsSearch, offset = 0) => {
  const params = new URLSearchParams({
    limit: String(FINDINGS_PAGE_SIZE),
    offset: String(offset),
  });
  const state = search.state ?? findingStateFilters.unresolved;
  if (state === findingStateFilters.unresolved)
    params.set('unresolved', 'true');
  else if (state !== findingStateFilters.all) params.set('state', state);
  if (search.severity) params.set('severity', search.severity);
  if (search.rule) params.set('rule_id', z.uuid().parse(search.rule));
  if (search.blueprint)
    params.set('blueprint_id', z.uuid().parse(search.blueprint));
  if (search.context) params.set('context_id', z.uuid().parse(search.context));
  return requestOffsetPage(
    `/api/rule-findings?${params}`,
    z.array(findingSchema),
  );
};
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

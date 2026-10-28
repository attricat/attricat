import { z } from 'zod';
import { request, requestOffsetPage, jsonRequest } from '../../api/request';
import {
  FINDINGS_PAGE_SIZE,
  findingStateFilters,
  RECORD_FINDINGS_PAGE_SIZE,
  RECORD_RULE_RUNS_LIMIT,
  RULE_PAGE_RUNS_LIMIT,
  runScopeFilters,
} from './constants';
import {
  compiledRuleSchema,
  createRuleSchema,
  findingSchema,
  ruleDetailsSchema,
  ruleRevisionSchema,
  ruleRunDetailsSchema,
  ruleRunSchema,
  ruleSchema,
  updateRuleSchema,
  type CreateRule,
  type FindingsSearch,
  type RunsSearch,
  type UpdateRule,
} from './schemas';
import { uuidSegment } from '../../api/pathSegments';
import { randomUuid } from '../../components/randomUuid';
export type {
  CompiledRule,
  CreateRule,
  Finding,
  FindingsSearch,
  Rule,
  RunsSearch,
  RuleDetails,
  RuleRevision,
  RuleRun,
  RuleRunDetails,
  RuleWarning,
  UpdateRule,
} from './schemas';
const rulePath = (ruleId: string) => `/api/rules/${uuidSegment(ruleId)}`;
/** A record's data quality, readable by everyone who may read the record. */
const recordPath = (recordId: string) =>
  `/api/v1/records/${uuidSegment(recordId)}`;
/** Every rule with the blueprint revisions it skips. */
export const listRules = () =>
  request('/api/rules', z.array(ruleDetailsSchema));
/** The rules of a record's blueprint, for naming the rules of its findings. */
export const listRecordRules = (recordId: string) =>
  request(`${recordPath(recordId)}/rules`, z.array(ruleDetailsSchema));
export const getRule = (ruleId: string) =>
  request(rulePath(ruleId), ruleDetailsSchema);
/** A rule's saved revisions, newest first: its changelog. */
export const listRuleRevisions = (ruleId: string) =>
  request(`${rulePath(ruleId)}/revisions`, z.array(ruleRevisionSchema));
/**
 * Runs the save checks on a rule against its blueprint's latest published
 * revision without storing it.
 */
export const validateRule = (input: CreateRule, signal?: AbortSignal) =>
  request('/api/rules/validate', compiledRuleSchema, {
    ...jsonRequest('POST', createRuleSchema.parse(input)),
    signal,
  });
/** Creates a disabled rule with its first revision. */
export const createRule = (input: CreateRule) =>
  request(
    '/api/rules',
    ruleSchema,
    jsonRequest('POST', createRuleSchema.parse(input)),
  );
/** Saves a changed definition as the rule's new current revision. */
export const updateRule = (ruleId: string, input: UpdateRule) =>
  request(
    rulePath(ruleId),
    ruleSchema,
    jsonRequest('PUT', updateRuleSchema.parse(input)),
  );
/** One run with, for a dry run, the first records it would flag. */
export const getRuleRun = (runId: string) =>
  request(`/api/rule-runs/${uuidSegment(runId)}`, ruleRunDetailsSchema);
/** The workspace's runs that match the run history's filters. */
export const listRuleRuns = (search: RunsSearch = {}) => {
  const params = new URLSearchParams();
  if (search.rule) params.set('rule_id', z.uuid().parse(search.rule));
  const scope = search.scope ?? runScopeFilters.allRecords;
  if (scope !== runScopeFilters.any) params.set('scope', scope);
  return request(`/api/rule-runs?${params}`, z.array(ruleRunSchema));
};
/** A rule's most recent runs, newest first. */
export const listRecentRuleRuns = (ruleId: string) =>
  request(
    `/api/rule-runs?${new URLSearchParams({
      rule_id: z.uuid().parse(ruleId),
      scope: runScopeFilters.allRecords,
      limit: String(RULE_PAGE_RUNS_LIMIT),
    })}`,
    z.array(ruleRunSchema),
  );
/** The most recent runs scoped to one record. */
export const listRecordRuleRuns = (recordId: string) =>
  request(
    `${recordPath(recordId)}/rule-runs?limit=${RECORD_RULE_RUNS_LIMIT}`,
    z.array(ruleRunSchema),
  );
/** Re-checks one record against every enabled rule of its blueprint. */
export const recheckRecord = (recordId: string) =>
  request(
    '/api/rule-findings/recheck',
    z.object({ run_ids: z.array(z.uuid()) }),
    jsonRequest('POST', {
      record_id: z.uuid().parse(recordId),
      idempotency_key: randomUuid(),
    }),
  );
/** The findings page's filters other than state, under their API names. */
const findingMatch = (search: FindingsSearch) => ({
  ...(search.severity ? { severity: search.severity } : {}),
  ...(search.rule ? { rule_id: z.uuid().parse(search.rule) } : {}),
  ...(search.blueprint
    ? { blueprint_id: z.uuid().parse(search.blueprint) }
    : {}),
  ...(search.context ? { context_id: z.uuid().parse(search.context) } : {}),
});
/** The findings page's filters as query parameters. */
const findingParams = (search: FindingsSearch) => {
  const params = new URLSearchParams(findingMatch(search));
  const state = search.state ?? findingStateFilters.unresolved;
  if (state === findingStateFilters.unresolved)
    params.set('unresolved', 'true');
  else if (state === findingStateFilters.all) params.set('unresolved', 'false');
  else params.set('state', state);
  return params;
};
/** One page of the workspace's findings that match the findings page's filters. */
export const listFindingsPage = (search: FindingsSearch, offset = 0) => {
  const params = findingParams(search);
  // Every state is the list's default.
  if (params.get('unresolved') === 'false') params.delete('unresolved');
  params.set('limit', String(FINDINGS_PAGE_SIZE));
  params.set('offset', String(offset));
  return requestOffsetPage(
    `/api/rule-findings?${params}`,
    z.array(findingSchema),
  );
};
/** How many findings match the findings page's filters. */
export const countFindings = (search: FindingsSearch) =>
  request(
    `/api/rule-findings/count?${findingParams(search)}`,
    z.object({ count: z.number().int().nonnegative() }),
  );
/**
 * Acknowledges the listed findings, or every open finding that matches the
 * findings page's filters, and says how many it acknowledged.
 */
export const acknowledgeFindings = (
  selection: { findingIds: readonly string[] } | { matching: FindingsSearch },
) =>
  request(
    '/api/rule-findings/acknowledge',
    z.object({ acknowledged: z.number().int().nonnegative() }),
    jsonRequest(
      'POST',
      'findingIds' in selection
        ? { finding_ids: selection.findingIds.map((id) => z.uuid().parse(id)) }
        : { matching: findingMatch(selection.matching) },
    ),
  );
/** One page of a record's open and acknowledged findings, newest change first. */
export const listUnresolvedRecordFindings = (recordId: string, offset = 0) =>
  requestOffsetPage(
    `${recordPath(recordId)}/rule-findings?${new URLSearchParams({
      limit: String(RECORD_FINDINGS_PAGE_SIZE),
      offset: String(offset),
    })}`,
    z.array(findingSchema),
  );
/** How many of a record's findings are open or acknowledged. */
export const getUnresolvedRecordFindingCount = (recordId: string) =>
  request(
    `${recordPath(recordId)}/rule-findings/count`,
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
/**
 * Queues a run of the rule's current revision. A dry run may check an unsaved
 * `definition` instead, so an enforcing change can be dry-run before saving.
 */
export const runRuleNow = (
  ruleId: string,
  dryRun: boolean,
  definition?: string,
) =>
  request(
    `${rulePath(ruleId)}/run-now`,
    z.object({ id: z.uuid() }),
    jsonRequest('POST', {
      dry_run: dryRun,
      record_id: null,
      idempotency_key: randomUuid(),
      ...(definition === undefined ? {} : { definition }),
    }),
  );
export const enableRule = (ruleId: string, acceptExistingViolations = false) =>
  request(
    `${rulePath(ruleId)}/enable`,
    ruleSchema,
    jsonRequest(
      'POST',
      acceptExistingViolations ? { accept_existing_violations: true } : {},
    ),
  );
export const disableRule = (ruleId: string) =>
  request(`${rulePath(ruleId)}/disable`, ruleSchema, {
    method: 'POST',
  });

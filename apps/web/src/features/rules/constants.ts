import type { ApiErrorCode } from '../../api/errors';

export const RULE_SECTION_RULES = 'rules';
export const RULE_SECTION_FINDINGS = 'findings';
export const RULE_SECTION_RUNS = 'runs';

export const RULE_SECTIONS = [
  { key: RULE_SECTION_RULES, to: '/manage/rules' },
  { key: RULE_SECTION_FINDINGS, to: '/manage/rules/findings' },
  { key: RULE_SECTION_RUNS, to: '/manage/rules/runs' },
] as const;

export type RuleInspectionSection = (typeof RULE_SECTIONS)[number]['key'];

export const findingStates = {
  open: 'open',
  acknowledged: 'acknowledged',
  resolved: 'resolved',
} as const;
/**
 * Which findings the findings page lists by state. Unresolved, the default,
 * is open and acknowledged findings together.
 */
export const findingStateFilters = {
  unresolved: 'unresolved',
  ...findingStates,
  all: 'all',
} as const;
/** Findings the findings page lists at a time. */
export const FINDINGS_PAGE_SIZE = 50;
/** The value of a findings filter that is not set. */
export const ANY_FINDING_FILTER = '';
/** Keeps the findings filters readable when their labels are short. */
export const FINDING_FILTER_MIN_WIDTH = 180;
export const findingSeverities = {
  info: 'info',
  warning: 'warning',
  error: 'error',
  critical: 'critical',
} as const;
export const ruleTriggerTypes = {
  manual: 'manual',
  schedule: 'schedule',
  event: 'event',
  postImport: 'post_import',
} as const;
export const ruleRoutes = {
  list: '/manage/rules',
  create: '/manage/rules/new',
  revision: '/manage/rules/$ruleId/revisions/$version',
  newRevision: '/manage/rules/$ruleId/revisions/$version/new',
} as const;
/** Keeps the rule scope selectors readable when their labels are short. */
export const RULE_SCOPE_SELECT_MIN_WIDTH = 220;
/** The value of the context selector for a rule that checks every context. */
export const ALL_CONTEXTS = '';
/** Height of the read-only definition on a rule revision page. */
export const RULE_DEFINITION_VIEWER_HEIGHT = '420px';
/** Recent runs of one record that the record page inspects. */
export const RECORD_RULE_RUNS_LIMIT = 20;
/** How often the record page checks on its rule runs while they are active. */
export const RECORD_RULE_RUN_POLL_MS = 1000;
/**
 * How long after an edit the record page waits for the rule runs that the
 * edit's events start; they are created asynchronously.
 */
export const RECORD_RULE_CHECK_WATCH_MS = 15_000;
export const RULE_HAS_EXISTING_VIOLATIONS =
  'rule_has_existing_violations' satisfies ApiErrorCode;
/** Columns of the findings table, for its no-matches row. */
export const findingsTableColumnCount = 6;
/** Unresolved findings the record page lists at a time. */
export const RECORD_FINDINGS_PAGE_SIZE = 20;

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

export const RULE_TAB_ID_PREFIX = 'rule-inspection-tab';
export const RULE_TABPANEL_ID_PREFIX = 'rule-inspection-tabpanel';

export const FINDING_STATE_OPEN = 'open';
export const FINDING_STATE_ACKNOWLEDGED = 'acknowledged';
export const FINDING_STATE_RESOLVED = 'resolved';
/**
 * Which findings the findings page lists by state. Unresolved, the default,
 * is open and acknowledged findings together.
 */
export const findingStateFilters = {
  unresolved: 'unresolved',
  open: FINDING_STATE_OPEN,
  acknowledged: FINDING_STATE_ACKNOWLEDGED,
  resolved: FINDING_STATE_RESOLVED,
  all: 'all',
} as const;
export type FindingStateFilter =
  (typeof findingStateFilters)[keyof typeof findingStateFilters];
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
/** Run statuses that have not finished evaluating. */
export const ACTIVE_RULE_RUN_STATUSES: readonly string[] = [
  'pending',
  'leased',
];
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
/** Characters of a record ID shown where no display label is loaded. */
export const SHORT_RECORD_ID_LENGTH = 8;
/** Unresolved findings the record page lists at a time. */
export const RECORD_FINDINGS_PAGE_SIZE = 20;

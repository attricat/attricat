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
export const FINDING_STATE_RESOLVED = 'resolved';
export const ERROR_FINDING_SEVERITIES: readonly string[] = [
  'error',
  'critical',
];
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
export const RULE_STATUS_PUBLISHED = 'published';
export const RULE_HAS_EXISTING_VIOLATIONS = 'rule_has_existing_violations';
/** Characters of a record ID shown where no display label is loaded. */
export const SHORT_RECORD_ID_LENGTH = 8;
/** The record's findings panel, which the data quality chip moves to. */
export const recordFindingsPanelId = (recordId: string) =>
  `record-findings-${recordId}`;

import type { ApiErrorCode } from '../../api/errors';

export const RULE_SECTION_RULES = 'rules';
export const RULE_SECTION_FINDINGS = 'findings';
export const RULE_SECTION_RUNS = 'runs';

export const ruleRoutes = {
  list: '/manage/rules',
  findings: '/manage/rules/findings',
  runs: '/manage/rules/runs',
  create: '/manage/rules/new',
  rule: '/manage/rules/$ruleId',
  changelog: '/manage/rules/$ruleId/changelog',
  edit: '/manage/rules/$ruleId/edit',
} as const;

export const RULE_PAGE_DEFINITION = 'definition';
export const RULE_PAGE_CHANGELOG = 'changelog';
/** The tabs of a rule's page. */
export const RULE_PAGE_SECTIONS = [
  { key: RULE_PAGE_DEFINITION, to: ruleRoutes.rule },
  { key: RULE_PAGE_CHANGELOG, to: ruleRoutes.changelog },
] as const;
export type RulePageSection = (typeof RULE_PAGE_SECTIONS)[number]['key'];

/** Why a rule does not apply to a blueprint revision. */
export const ruleInapplicabilities = {
  attributeMissing: 'attribute_missing',
  attributeTypeChanged: 'attribute_type_changed',
  statusChanged: 'status_changed',
} as const;

export const RULE_SECTIONS = [
  { key: RULE_SECTION_RULES, to: ruleRoutes.list },
  { key: RULE_SECTION_FINDINGS, to: ruleRoutes.findings },
  { key: RULE_SECTION_RUNS, to: ruleRoutes.runs },
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
/**
 * Records by their unresolved findings, as Explorer filters them. Every
 * record falls in exactly one.
 */
export const dataQualityFilters = {
  /** At least one open finding. */
  open: 'open',
  /** Unresolved findings, every one acknowledged. */
  acknowledged: 'acknowledged',
  /** No unresolved findings. */
  clean: 'clean',
} as const;
export type DataQualityFilter =
  (typeof dataQualityFilters)[keyof typeof dataQualityFilters];
export const ruleTriggerTypes = {
  manual: 'manual',
  schedule: 'schedule',
  event: 'event',
  postImport: 'post_import',
} as const;
/** The `type` of each predicate a compiled rule can check. */
export const predicateTypes = {
  required: 'required',
  stale: 'stale',
  hasTag: 'has_tag',
  missingTag: 'missing_tag',
  compare: 'compare',
  oneOf: 'one_of',
  relativeDate: 'relative_date',
  unique: 'unique',
  linked: 'linked',
  referencedBy: 'referenced_by',
  acyclic: 'acyclic',
  allOf: 'all_of',
  anyOf: 'any_of',
} as const;
/** Keeps the rule scope selectors readable when their labels are short. */
export const RULE_SCOPE_SELECT_MIN_WIDTH = 220;
/** The value of the context selector for a rule that checks every context. */
export const ALL_CONTEXTS = '';
/** Height of the read-only definition on a rule's page. */
export const RULE_DEFINITION_VIEWER_HEIGHT = '420px';
/** How often an editor checks on the dry run of its unsaved change. */
export const RULE_DRY_RUN_POLL_MS = 1000;
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
export const RULE_DRY_RUN_REQUIRED =
  'rule_dry_run_required' satisfies ApiErrorCode;
/** Columns of the findings table, for its no-matches row. */
export const findingsTableColumnCount = 6;
/** Unresolved findings the record page lists at a time. */
export const RECORD_FINDINGS_PAGE_SIZE = 20;

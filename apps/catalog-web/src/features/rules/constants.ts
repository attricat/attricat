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

import type { Rule } from './api';

/** Rule revisions by `id:version`, for naming the rule of a run or finding. */
export type RuleRevisions = ReadonlyMap<string, Rule>;

export const ruleRevisionKey = (ruleId: string, version: number) =>
  `${ruleId}:${version}`;

export const ruleRevisionsByKey = (rules: readonly Rule[]): RuleRevisions =>
  new Map(rules.map((rule) => [ruleRevisionKey(rule.id, rule.version), rule]));

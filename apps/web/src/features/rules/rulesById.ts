import type { Rule } from './api';

/** Rules by id, for naming the rule of a run or finding. */
export type RulesById = ReadonlyMap<string, Rule>;

export const rulesById = (rules: readonly Rule[]): RulesById =>
  new Map(rules.map((rule) => [rule.id, rule]));

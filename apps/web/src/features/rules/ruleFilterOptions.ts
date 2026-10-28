import type { Rule } from './api';

/** One choice of a list filter. */
export type FilterOption = { value: string; label: string };

/** Each rule, by name. */
export const ruleFilterOptions = (rules: readonly Rule[]): FilterOption[] =>
  rules
    .map((rule) => ({ value: rule.id, label: rule.name }))
    .sort((a, b) => a.label.localeCompare(b.label));

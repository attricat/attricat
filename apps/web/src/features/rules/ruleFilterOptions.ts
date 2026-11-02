import { lexiconText } from '../lexicon/lexicon';
import type { Rule } from './api';

/** One choice of a list filter. */
export type FilterOption = { value: string; label: string };

const byLabel = (a: FilterOption, b: FilterOption) =>
  a.label.localeCompare(b.label);

/** Each rule, by name. */
export const ruleFilterOptions = (rules: readonly Rule[]): FilterOption[] =>
  rules.map((rule) => ({ value: rule.id, label: rule.name })).sort(byLabel);

/** Each blueprint, by translated name. */
export const blueprintFilterOptions = (
  blueprints: readonly { id: string; name: string }[],
): FilterOption[] =>
  blueprints
    .map((blueprint) => ({
      value: blueprint.id,
      label: lexiconText(blueprint.name),
    }))
    .sort(byLabel);

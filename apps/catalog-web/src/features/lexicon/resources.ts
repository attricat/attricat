import {
  CONTEXT_SEPARATOR,
  DEFAULT_PLURAL_CATEGORY,
  PLURAL_SEPARATOR,
  SINGULAR_PLURAL_CATEGORY,
} from './constants';
import type { LexiconReference } from './references';
import type { LexiconEntry } from './schemas';

/** `Order|purchase` is stored as `Order_purchase`, i18next's context form. */
export const lexiconResourceKey = ({ key, context }: LexiconReference) =>
  context ? `${key}${CONTEXT_SEPARATOR}${context}` : key;

export const pluralResourceKey = (resourceKey: string, category: string) =>
  `${resourceKey}${PLURAL_SEPARATOR}${category}`;

/**
 * Builds a flat i18next resource bundle for one language. An entry set with
 * plural forms gets suffixed keys (`Product_few`); its bare key, used by text
 * without a count, takes the singular form because keys are singular.
 */
export const lexiconResources = (entries: LexiconEntry[]) => {
  const groups = new Map<string, Map<string, string>>();
  for (const entry of entries) {
    const resourceKey = lexiconResourceKey({
      key: entry.key,
      context: entry.context ?? undefined,
    });
    const forms = groups.get(resourceKey) ?? new Map<string, string>();
    forms.set(entry.plural_category, entry.text);
    groups.set(resourceKey, forms);
  }
  const resources: Record<string, string> = {};
  for (const [resourceKey, forms] of groups) {
    const pluralForms = [...forms.keys()].some(
      (category) => category !== DEFAULT_PLURAL_CATEGORY,
    );
    if (pluralForms)
      for (const [category, text] of forms)
        resources[pluralResourceKey(resourceKey, category)] = text;
    const bare =
      forms.get(SINGULAR_PLURAL_CATEGORY) ??
      forms.get(DEFAULT_PLURAL_CATEGORY) ??
      forms.values().next().value;
    if (bare !== undefined) resources[resourceKey] = bare;
  }
  return resources;
};

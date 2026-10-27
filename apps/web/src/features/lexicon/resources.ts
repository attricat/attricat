import {
  CONTEXT_SEPARATOR,
  DEFAULT_PLURAL_CATEGORY,
  PLAIN_KEY_RESOURCE_PREFIX,
  PLURAL_SEPARATOR,
  SINGULAR_PLURAL_CATEGORY,
} from './constants';
import type { LexiconReference } from './references';
import type { LexiconEntry } from './schemas';

/** `Order|purchase` is stored as `Order_purchase`, i18next's context form. */
export const lexiconResourceKey = ({ key, context }: LexiconReference) =>
  context ? `${key}${CONTEXT_SEPARATOR}${context}` : key;

/** Where the bundle keeps a context-free entry for whole-key lookups. */
export const plainKeyResourceKey = (key: string) =>
  `${PLAIN_KEY_RESOURCE_PREFIX}${key}`;

export const pluralResourceKey = (resourceKey: string, category: string) =>
  `${resourceKey}${PLURAL_SEPARATOR}${category}`;

/** The form text without a count uses: singular, because keys are singular. */
const bareForm = (forms: ReadonlyMap<string, string>) =>
  forms.get(SINGULAR_PLURAL_CATEGORY) ??
  forms.get(DEFAULT_PLURAL_CATEGORY) ??
  forms.values().next().value;

const formsByKey = (
  entries: readonly LexiconEntry[],
  keyOf: (entry: LexiconEntry) => string,
) => {
  const groups = new Map<string, Map<string, string>>();
  for (const entry of entries) {
    const key = keyOf(entry);
    const forms = groups.get(key) ?? new Map<string, string>();
    forms.set(entry.plural_category, entry.text);
    groups.set(key, forms);
  }
  return groups;
};

/**
 * Builds a flat i18next resource bundle for one language. An entry set with
 * plural forms gets suffixed keys (`Product_few`); its bare key, used by text
 * without a count, takes the singular form. Each context-free entry's bare
 * form is also kept under its plain key for whole-key lookups.
 */
export const lexiconResources = (entries: LexiconEntry[]) => {
  const resources: Record<string, string> = {};
  const groups = formsByKey(entries, (entry) =>
    lexiconResourceKey({
      key: entry.key,
      context: entry.context ?? undefined,
    }),
  );
  for (const [resourceKey, forms] of groups) {
    const pluralForms = [...forms.keys()].some(
      (category) => category !== DEFAULT_PLURAL_CATEGORY,
    );
    if (pluralForms)
      for (const [category, text] of forms)
        resources[pluralResourceKey(resourceKey, category)] = text;
    const bare = bareForm(forms);
    if (bare !== undefined) resources[resourceKey] = bare;
  }
  const plain = formsByKey(
    entries.filter((entry) => !entry.context),
    (entry) => entry.key,
  );
  for (const [key, forms] of plain) {
    const bare = bareForm(forms);
    if (bare !== undefined) resources[plainKeyResourceKey(key)] = bare;
  }
  return resources;
};

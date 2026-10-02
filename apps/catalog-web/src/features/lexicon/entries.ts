import { DEFAULT_PLURAL_CATEGORY } from './constants';
import type { StoredLexiconEntry } from './schemas';

/** Form values for one translation in the lexicon management dialogs. */
export type LexiconEntryDraft = {
  key: string;
  context: string;
  plural_category: string;
  text: string;
};

export const emptyLexiconEntryDraft: LexiconEntryDraft = {
  key: '',
  context: '',
  plural_category: DEFAULT_PLURAL_CATEGORY,
  text: '',
};

/** Stable row identity: `(key, context, plural_category)` within a language. */
export const lexiconEntryId = (
  entry: Pick<StoredLexiconEntry, 'key' | 'context' | 'plural_category'>,
) => JSON.stringify([entry.key, entry.context, entry.plural_category]);

/** A reference's identity, matching the report's orphaned entries. */
export const lexiconReferenceId = (key: string, context: string | null) =>
  JSON.stringify([key, context]);

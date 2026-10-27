import { afterEach, describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { LEXICON_NAMESPACE } from '../lexicon/constants';
import { lexiconResources } from '../lexicon/resources';
import type { LexiconEntry } from '../lexicon/schemas';
import { NEEDS_ATTENTION_SYSTEM_TAG } from './constants';
import { systemTagLabel } from './systemTags';

const entry = (
  language: string,
  key: string,
  text: string,
  { context = null, plural = 'other' } = {} as {
    context?: string | null;
    plural?: string;
  },
): LexiconEntry => ({
  key,
  context,
  language,
  plural_category: plural,
  text,
});

const load = (language: string, entries: LexiconEntry[]) =>
  i18n.addResourceBundle(
    language,
    LEXICON_NAMESPACE,
    lexiconResources(entries),
    false,
    true,
  );

describe('systemTagLabel', () => {
  afterEach(async () => {
    i18n.removeResourceBundle('pl', LEXICON_NAMESPACE);
    i18n.removeResourceBundle('en', LEXICON_NAMESPACE);
    await i18n.changeLanguage('en');
  });

  it('looks up a plain tag as a lexicon key and resolves references', async () => {
    load('pl', [
      entry('pl', 'workflow.needs-review', 'Do przeglądu'),
      entry('pl', 'Open', 'Otwarte', { context: 'ticket' }),
    ]);
    load('en', [entry('en', 'imported', 'Imported')]);
    await i18n.changeLanguage('pl');

    expect(systemTagLabel('workflow.needs-review')).toBe('Do przeglądu');
    expect(systemTagLabel('{{Open|ticket}}')).toBe('Otwarte');
    // Missing Polish text falls back to English, then to the tag.
    expect(systemTagLabel('imported')).toBe('Imported');
    expect(systemTagLabel('ext:batch|42')).toBe('ext:batch|42');
    expect(systemTagLabel('{{broken')).toBe('{{broken');
  });

  it('translates Core tags unless the workspace lexicon names them', async () => {
    await i18n.changeLanguage('pl');
    expect(systemTagLabel(NEEDS_ATTENTION_SYSTEM_TAG)).toBe('Wymaga uwagi');

    load('pl', [entry('pl', NEEDS_ATTENTION_SYSTEM_TAG, 'Do poprawy')]);
    expect(systemTagLabel(NEEDS_ATTENTION_SYSTEM_TAG)).toBe('Do poprawy');

    await i18n.changeLanguage('en');
    expect(systemTagLabel(NEEDS_ATTENTION_SYSTEM_TAG)).toBe('Needs attention');
    // A Core tag the app does not know shows as stored.
    expect(systemTagLabel('attricat.future')).toBe('attricat.future');
  });

  it('matches only a whole entry key, not a context or plural form', async () => {
    load('en', [
      entry('en', 'needs', 'Needs', { context: 'review' }),
      entry('en', 'x', 'One x', { plural: 'one' }),
      entry('en', 'x', 'Many x', { plural: 'other' }),
    ]);
    await i18n.changeLanguage('en');
    expect(systemTagLabel('needs_review')).toBe('needs_review');
    expect(systemTagLabel('x_one')).toBe('x_one');
    expect(systemTagLabel('x_other')).toBe('x_other');
    // The entry itself still names its tag.
    expect(systemTagLabel('x')).toBe('One x');
  });
});

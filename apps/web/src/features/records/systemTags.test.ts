import { afterEach, describe, expect, it } from 'vitest';
import i18n from '../../i18n';
import { LEXICON_NAMESPACE } from '../lexicon/constants';
import { NEEDS_ATTENTION_SYSTEM_TAG } from './constants';
import { systemTagLabel } from './systemTags';

describe('systemTagLabel', () => {
  afterEach(async () => {
    i18n.removeResourceBundle('pl', LEXICON_NAMESPACE);
    i18n.removeResourceBundle('en', LEXICON_NAMESPACE);
    await i18n.changeLanguage('en');
  });

  it('looks up a plain tag as a lexicon key and resolves references', async () => {
    i18n.addResourceBundle('pl', LEXICON_NAMESPACE, {
      'workflow.needs-review': 'Do przeglądu',
      Open_ticket: 'Otwarte',
    });
    i18n.addResourceBundle('en', LEXICON_NAMESPACE, { imported: 'Imported' });
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

    i18n.addResourceBundle('pl', LEXICON_NAMESPACE, {
      [NEEDS_ATTENTION_SYSTEM_TAG]: 'Do poprawy',
    });
    expect(systemTagLabel(NEEDS_ATTENTION_SYSTEM_TAG)).toBe('Do poprawy');

    await i18n.changeLanguage('en');
    expect(systemTagLabel(NEEDS_ATTENTION_SYSTEM_TAG)).toBe('Needs attention');
    // A Core tag the app does not know shows as stored.
    expect(systemTagLabel('attricat.future')).toBe('attricat.future');
  });
});

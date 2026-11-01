import i18n from 'i18next';
import { afterEach, describe, expect, it } from 'vitest';
import '../../i18n';
import { blueprintKindLabel } from './constants';

describe('blueprintKindLabel', () => {
  afterEach(() => i18n.changeLanguage('en'));

  it('translates known kinds and shows others as they are', async () => {
    expect(blueprintKindLabel(i18n.t, 'mixin')).toBe('Mixin');
    await i18n.changeLanguage('pl');
    expect(blueprintKindLabel(i18n.t, 'record')).toBe('Rekord');
    expect(blueprintKindLabel(i18n.t, 'catalogue')).toBe('catalogue');
  });
});

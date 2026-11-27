// @vitest-environment jsdom
import { afterEach, describe, expect, it } from 'vitest';
import i18n, {
  applyInstallationLanguage,
  chooseLanguage,
  languageStorageKey,
} from '.';

describe('the installation default language', () => {
  afterEach(async () => {
    localStorage.clear();
    await i18n.changeLanguage('en');
  });

  it('replaces the browser language until the person picks one', async () => {
    await applyInstallationLanguage('pl');
    expect(i18n.resolvedLanguage).toBe('pl');
    // Applying it is not a pick, so a later default still applies.
    expect(localStorage.getItem(languageStorageKey)).toBeNull();
  });

  it('never overrides a language the person picked', async () => {
    await chooseLanguage('en');
    expect(localStorage.getItem(languageStorageKey)).toBe('en');
    await applyInstallationLanguage('pl');
    expect(i18n.resolvedLanguage).toBe('en');
  });

  it('ignores the language older versions stored on every visit', async () => {
    localStorage.setItem('attricat.language', 'en');
    await applyInstallationLanguage('pl');
    expect(i18n.resolvedLanguage).toBe('pl');
  });

  it('leaves the language alone without a default', async () => {
    await applyInstallationLanguage(null);
    expect(i18n.resolvedLanguage).toBe('en');
  });
});

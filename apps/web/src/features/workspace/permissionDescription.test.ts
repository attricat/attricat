import i18n from 'i18next';
import { afterEach, describe, expect, it } from 'vitest';
import '../../i18n';
import { permissionDescription } from './permissionDescription';

describe('permissionDescription', () => {
  afterEach(() => i18n.changeLanguage('en'));

  it('translates known permissions and keeps the server text for others', async () => {
    await i18n.changeLanguage('pl');
    expect(
      permissionDescription(i18n.t, {
        code: 'records.read',
        description: 'Read catalog records',
      }),
    ).toBe('Odczyt rekordów katalogu');
    expect(
      permissionDescription(i18n.t, {
        code: 'reports.export',
        description: 'Export reports',
      }),
    ).toBe('Export reports');
  });
});

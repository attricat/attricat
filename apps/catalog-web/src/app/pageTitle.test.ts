import { describe, expect, it } from 'vitest';
import i18n from '../i18n';
import { pageTitle } from './pageTitle';

describe('pageTitle', () => {
  it('distinguishes routes and resources across browser tabs', () => {
    expect(pageTitle('/', i18n.t)).toBe('Entity explorer · Attricat');
    expect(pageTitle('/manage/workflows', i18n.t)).toBe('Workflows · Attricat');
    expect(pageTitle('/manage/workflows/new', i18n.t)).toBe('New workflow · Attricat');
    expect(pageTitle('/entities/abcdef12-1234/edit', i18n.t)).toBe(
      'Edit entity · Entity abcdef12 · Attricat',
    );
    expect(pageTitle('/entities/87654321-1234/edit', i18n.t)).not.toBe(
      pageTitle('/entities/abcdef12-1234/edit', i18n.t),
    );
    expect(pageTitle('/manage/workspace/members', i18n.t)).toBe(
      'Members · Workspace management · Attricat',
    );
  });

  it('translates route labels when the language changes', async () => {
    await i18n.changeLanguage('pl');
    expect(pageTitle('/manage/workflows', i18n.t)).toContain('Attricat');
    expect(pageTitle('/manage/workflows', i18n.t)).not.toBe('Workflows · Attricat');
    await i18n.changeLanguage('en');
  });
});

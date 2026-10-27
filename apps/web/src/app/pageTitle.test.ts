import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import i18n from '../i18n';
import { pageTitle } from './pageTitle';

describe('pageTitle', () => {
  it('distinguishes routes and resources across browser tabs', () => {
    expect(pageTitle('/', i18n.t)).toBe('Record explorer · Attricat');
    expect(pageTitle('/manage/workflows', i18n.t)).toBe('Workflows · Attricat');
    expect(pageTitle('/manage/workflows/new', i18n.t)).toBe(
      'New workflow · Attricat',
    );
    expect(pageTitle('/records/abcdef12-1234/changes', i18n.t)).toBe(
      'Changes · Record abcdef12 · Attricat',
    );
    expect(pageTitle('/records/87654321-1234/changes', i18n.t)).not.toBe(
      pageTitle('/records/abcdef12-1234/changes', i18n.t),
    );
    expect(pageTitle('/manage/workspace/members', i18n.t)).toBe(
      'Members · Workspace management · Attricat',
    );
    expect(pageTitle('/inbox', i18n.t)).toBe('Inbox · Attricat');
    expect(pageTitle('/manage/lexicon', i18n.t)).toBe(
      `${i18n.t('navigation.lexicon')} · Attricat`,
    );
  });

  it('translates route labels when the language changes', async () => {
    await i18n.changeLanguage('pl');
    expect(pageTitle('/manage/workflows', i18n.t)).toContain('Attricat');
    expect(pageTitle('/manage/workflows', i18n.t)).not.toBe(
      'Workflows · Attricat',
    );
    await i18n.changeLanguage('en');
  });
});

describe('pageTitle for every route', () => {
  // The generated route tree lists each full path; a route this file does not
  // know would otherwise silently show the not-found title.
  const routeTreeSource = readFileSync(
    fileURLToPath(new URL('../routeTree.gen.ts', import.meta.url)),
    'utf8',
  );
  const byFullPath = routeTreeSource.slice(
    routeTreeSource.indexOf('interface FileRoutesByFullPath'),
  );
  const paths = [
    ...new Set(
      [
        ...byFullPath
          .slice(0, byFullPath.indexOf('}'))
          .matchAll(/^\s+'([^']+)':/gm),
      ].map(([, path]) => path),
    ),
  ];
  const samplePath = (path: string) =>
    path.replace(/\$[A-Za-z]+/g, '123e4567-e89b-12d3-a456-426614174000');

  it.each(paths)('names %s', (path) => {
    expect(paths.length).toBeGreaterThan(50);
    expect(pageTitle(samplePath(path), i18n.t)).not.toContain(
      i18n.t('errors.notFoundTitle'),
    );
  });
});

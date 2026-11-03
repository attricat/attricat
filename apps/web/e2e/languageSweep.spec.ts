import { readFileSync } from 'node:fs';
import { expect, test, type Page } from '@playwright/test';
import { createRecord, createRecordBlueprint, scalar, suffix } from './helpers';

const read = (locale: string) =>
  JSON.parse(
    readFileSync(
      new URL(`../src/i18n/locales/${locale}.json`, import.meta.url),
      'utf8',
    ),
  );
const flatten = (value: unknown, path = ''): Array<[string, string]> =>
  typeof value === 'string'
    ? [[path, value]]
    : Object.entries(value as object).flatMap(([key, child]) =>
        flatten(child, path ? `${path}.${key}` : key),
      );
const english = new Map(flatten(read('en')));
const polish = new Map(flatten(read('pl')));
/** English sentences, which on a Polish page mean a missed translation. */
const untranslated = new Set(
  [...english]
    .filter(
      ([key, value]) =>
        value.includes(' ') &&
        !value.includes('{{') &&
        polish.has(key) &&
        polish.get(key) !== value,
    )
    .map(([, value]) => value),
);

const staticPages = [
  '/',
  '/inbox',
  '/agents',
  '/profile',
  '/profile/extension-runs',
  '/profile/personal-access-tokens',
  '/manage',
  '/manage/audit-log',
  '/manage/background-processing',
  '/manage/blueprints',
  '/manage/blueprints/new',
  '/manage/contexts',
  '/manage/contexts/new',
  '/manage/data-health',
  '/manage/exports',
  '/manage/extensions/installed',
  '/manage/extensions/layout',
  '/manage/extensions/marketplace',
  '/manage/extensions/sideload',
  '/manage/lexicon',
  '/manage/reusable-attributes',
  '/manage/reusable-attributes/new',
  '/manage/rules',
  '/manage/rules/findings',
  '/manage/rules/new',
  '/manage/rules/runs',
  '/manage/system-health',
  '/manage/workflows',
  '/manage/workflows/new',
  '/manage/workspace/agent',
  '/manage/workspace/invitations',
  '/manage/workspace/members',
  '/manage/workspace/navigation',
  '/manage/workspace/roles',
  '/manage/workspace/sign-in',
  '/manage/workspace/tag-colors',
  '/manage/workspace/teams',
];

const visibleTexts = (page: Page) =>
  page.evaluate(() => {
    const walker = document.createTreeWalker(
      document.body,
      NodeFilter.SHOW_TEXT,
    );
    const texts: string[] = [];
    while (walker.nextNode()) {
      const text = walker.currentNode.textContent?.trim();
      const parent = walker.currentNode.parentElement;
      if (text && parent && !parent.closest('script, style, .monaco-editor'))
        texts.push(text);
    }
    return texts;
  });

for (const locale of ['en', 'pl'] as const) {
  test(`every page renders in ${locale}`, async ({ page }, testInfo) => {
    test.setTimeout(240_000);
    const blueprint = await createRecordBlueprint(
      `sweep_${suffix()}`,
      'Sweep product',
      'attributes:\n  - code: title\n    value_type: string',
    );
    const record = await createRecord(blueprint, [scalar('title', 'Sweep')]);
    await page.addInitScript((language) => {
      // This also runs in extensions' sandboxed frames, which have no storage.
      if (window.origin !== 'null')
        localStorage.setItem('attricat.language', language);
    }, locale);
    const problems: string[] = [];
    let current = '';
    page.on('pageerror', (error) =>
      problems.push(`${current}: exception ${error.message}`),
    );
    page.on('console', (message) => {
      if (
        message.type() === 'error' &&
        !message.text().includes('Failed to load resource')
      )
        problems.push(`${current}: console ${message.text()}`);
    });
    page.on('response', (response) => {
      if (
        response.status() >= 500 &&
        new URL(response.url()).pathname.startsWith('/api/')
      )
        problems.push(`${current}: ${response.status()} ${response.url()}`);
    });
    const pages = [
      ...staticPages,
      `/records/${record.id}`,
      `/records/${record.id}/changes`,
      `/manage/blueprints/${blueprint.blueprint.id}`,
      '/records/new',
    ];
    const directory = process.env.E2E_SWEEP_SCREENSHOTS;
    for (const path of pages) {
      current = path;
      await page.goto(path);
      await page.waitForLoadState('networkidle');
      await expect(page.locator('body')).not.toBeEmpty();
      for (const text of await visibleTexts(page)) {
        if (english.has(text))
          problems.push(`${path}: raw translation key "${text}"`);
        if (locale === 'pl' && untranslated.has(text))
          problems.push(`${path}: English text "${text}"`);
      }
      if (directory)
        await page.screenshot({
          fullPage: true,
          path: `${directory}/${locale}${path.replaceAll('/', '_')}.png`,
        });
    }
    await testInfo.attach('problems', { body: problems.join('\n') });
    expect(problems).toEqual([]);
  });
}

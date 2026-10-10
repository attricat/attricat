import { expect, test } from '@playwright/test';
import {
  commitField,
  createRecord,
  createRecordBlueprint,
  scalar,
  suffix,
} from './helpers';

for (const mode of ['light', 'dark'] as const) {
  test(`URL editing and safe detail/table rendering (${mode})`, async ({
    page,
  }) => {
    await page.addInitScript(
      (value) => localStorage.setItem('attricat.color-mode', value),
      mode,
    );
    const code = `url_${suffix()}`;
    const blueprint = await createRecordBlueprint(
      code,
      'URL controls',
      `
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "website"
value_type = "string"
`,
      {
        views: `
[views.detail]
type = "stack"
children = [{ type = "field", field = "website", component = { id = "catalog.url_display", version = 1 } }]
[views.table]
type = "table"
columns = [{ field = "website", renderer = { id = "catalog.url_display", version = 1 } }]
`,
      },
    );
    const record = await createRecord(blueprint, [
      scalar('title', 'URL example'),
      scalar('website', 'javascript:alert(1)'),
    ]);
    // The record page edits the value in place; the table displays it.
    await page.goto(`/?blueprint=${code}`);
    await expect(
      page.getByText('javascript:alert(1)', { exact: true }),
    ).toBeVisible();
    await expect(page.locator('a[href^="javascript:"]')).toHaveCount(0);
    await page.goto(`/records/${record.id}`);
    const input = page.getByRole('textbox', { name: 'website' });
    await expect(input).toHaveAttribute('type', 'url');
    await expect(input).toHaveValue('javascript:alert(1)');
    await expect(page.locator('a[href^="javascript:"]')).toHaveCount(0);
    const saves: string[] = [];
    page.on('request', (request) => {
      if (request.method() === 'PUT') saves.push(request.url());
    });
    // An unsafe URL stays a local edit with its error and is not sent.
    await input.fill('ftp://example.com/file');
    await input.press('Enter');
    await expect(input).toHaveAttribute('aria-invalid', 'true');
    await expect(input).toHaveValue('ftp://example.com/file');
    expect(saves).toEqual([]);
    await input.fill('https://example.com/a?query=1#section');
    await commitField(page, record.id, input, 'Enter');
    await page.reload();
    await expect(input).toHaveValue('https://example.com/a?query=1#section');
    await page.goto(`/?blueprint=${code}`);
    const link = page.getByRole('link', { name: /https:\/\/example.com\/a/ });
    await expect(link).toHaveAttribute('target', '_blank');
    await expect(link).toHaveAttribute('rel', 'noopener noreferrer');
    // The explorer may move focus while it finishes loading.
    await expect(async () => {
      await link.focus();
      await expect(link).toBeFocused({ timeout: 1_000 });
    }).toPass();
    await page.goto(`/records/${record.id}`);
    await input.clear();
    await commitField(page, record.id, input);
    await page.reload();
    await expect(input).toHaveValue('');
    await page.goto(`/?blueprint=${code}`);
    await expect(
      page.getByRole('link', { name: /https:\/\/example.com\/a/ }),
    ).toHaveCount(0);
  });
}

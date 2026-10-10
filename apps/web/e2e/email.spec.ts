import { expect, test } from '@playwright/test';
import {
  commitField,
  createRecord,
  createRecordBlueprint,
  scalar,
  suffix,
} from './helpers';

for (const colorScheme of ['light', 'dark'] as const) {
  test(`email display and editing in ${colorScheme} mode`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    const code = `email_${suffix()}`;
    if (colorScheme === 'dark')
      await page.setViewportSize({ width: 390, height: 844 });
    const blueprint = await createRecordBlueprint(
      code,
      'Email contact',
      `
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "contact"
value_type = "string"
value_schema = '{"type":"string","format":"email"}'
`,
      {
        views: `
[views.detail]
type = "stack"
children = [{ type = "field", field = "contact", component = { id = "attricat.email_display", version = 1 } }]
[views.table]
type = "table"
columns = [{ field = "title" }, { field = "contact", renderer = { id = "attricat.email_display", version = 1 } }]
`,
      },
    );
    await expect(
      createRecord(blueprint, [
        scalar('title', 'Invalid contact'),
        scalar('contact', 'not an email'),
      ]),
    ).rejects.toThrow(/422/);
    const record = await createRecord(blueprint, [
      scalar('title', 'Contact'),
      scalar('contact', 'Name+tag@Example.com'),
    ]);
    // The record page edits the value in place; the table displays it.
    await page.goto(`/?blueprint=${code}`);
    await expect(
      page.getByRole('link', { name: 'Name+tag@Example.com' }),
    ).toHaveAttribute('href', 'mailto:Name%2Btag@Example.com');
    await page.goto(`/records/${record.id}`);
    const input = page.getByRole('textbox', { name: 'contact' });
    await expect(input).toHaveAttribute('type', 'email');
    await expect(input).toHaveValue('Name+tag@Example.com');
    const saves: string[] = [];
    page.on('request', (request) => {
      if (request.method() === 'PUT') saves.push(request.url());
    });
    // An invalid address stays a local edit with its error and is not sent.
    await input.fill('invalid');
    await input.press('Tab');
    await expect(input).toHaveAttribute('aria-invalid', 'true');
    await expect(input).toHaveValue('invalid');
    expect(saves).toEqual([]);
    await input.fill('Other+tag@Example.com');
    // Tab would move to the field's mail button; Enter commits in place.
    await commitField(page, record.id, input, 'Enter');
    await expect(input).not.toHaveAttribute('aria-invalid', 'true');
    await page.reload();
    await expect(input).toHaveValue('Other+tag@Example.com');
    await page.goto(`/?blueprint=${code}`);
    await expect(
      page.getByRole('link', { name: 'Other+tag@Example.com' }),
    ).toBeVisible();
    await page.goto(`/records/${record.id}`);
    await input.clear();
    await commitField(page, record.id, input, 'Enter');
    await page.reload();
    await expect(input).toHaveValue('');
    await page.goto(`/?blueprint=${code}`);
    await expect(
      page.getByRole('link', { name: 'Contact', exact: true }),
    ).toBeVisible();
    await expect(page.locator('a[href^="mailto:"]')).toHaveCount(0);
  });
}

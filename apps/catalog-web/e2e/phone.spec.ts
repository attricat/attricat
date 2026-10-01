import { expect, test } from '@playwright/test';
import { createEntityBlueprint, suffix } from './helpers';

for (const colorScheme of ['light', 'dark'] as const) {
  test(`creates, displays and edits phone values (${colorScheme})`, async ({
    page,
  }) => {
    await page.emulateMedia({ colorScheme });
    const code = `phone_${suffix()}`;
    await createEntityBlueprint(
      code,
      'Phone contacts',
      `
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "phone"
value_type = "string"
`,
      {
        views: `
[views.detail]
type = "stack"
children = [{ type = "field", field = "phone", component = { id = "catalog.phone_display", version = 1 } }]
[views.edit]
type = "stack"
children = [{ type = "field", field = "title" }, { type = "field", field = "phone", component = { id = "catalog.phone_edit", version = 1 } }]
[views.table]
type = "table"
columns = [{ field = "phone", renderer = { id = "catalog.phone_display", version = 1 } }]
`,
      },
    );
    await page.goto('/entities/new');
    const toggle = page.getByRole('button', {
      name: colorScheme === 'dark' ? 'Dark mode' : 'Light mode',
      exact: true,
    });
    if (await toggle.isVisible()) await toggle.click();
    await page
      .getByRole('combobox', { name: 'Blueprint', exact: true })
      .click();
    await page
      .getByRole('option', { name: `Phone contacts (${code})` })
      .click();
    await page.getByRole('button', { name: 'Load blueprint' }).click();
    await page.getByLabel('title', { exact: true }).fill('Phone contact');
    const input = page.getByLabel('phone', { exact: true });
    await expect(input).toHaveAttribute('type', 'tel');
    await expect(input).toHaveAttribute('inputmode', 'tel');
    const number = '+1 (202) 555-0123 x0042';
    await input.fill(number);
    await page
      .getByRole('button', { name: 'Create entity', exact: true })
      .click();
    await expect(page).toHaveURL(/\/entities\/[0-9a-f-]{36}$/);
    const entityUrl = page.url();
    await expect(
      page.getByRole('link', { name: `Call ${number}` }),
    ).toHaveAttribute('href', 'tel:+12025550123;ext=0042');
    await page.reload();
    await expect(
      page.getByRole('link', { name: `Call ${number}` }),
    ).toBeVisible();
    await page.goto(`/?blueprint=${code}`);
    await expect(
      page.getByRole('link', { name: `Call ${number}` }),
    ).toHaveAttribute('href', 'tel:+12025550123;ext=0042');
    await page.goto(`${entityUrl}/edit`);
    await expect(input).toHaveValue(number);
    await input.fill('020 7946 0958');
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page).toHaveURL(entityUrl);
    await expect(
      page.getByText('020 7946 0958', { exact: true }),
    ).toBeVisible();
    await expect(page.locator('a[href^="tel:"]')).toHaveCount(0);
    await page.goto(`${entityUrl}/edit`);
    await input.clear();
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page).toHaveURL(entityUrl);
    await page.reload();
    await expect(page.getByText('Not set', { exact: true })).toBeVisible();
    await expect(page.locator('a[href^="tel:"]')).toHaveCount(0);
  });
}

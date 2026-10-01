import { expect, test } from '@playwright/test';
import { createEntity, createEntityBlueprint, scalar, suffix } from './helpers';

for (const colorScheme of ['light', 'dark'] as const) {
  test(`email display and editing in ${colorScheme} mode`, async ({ page }) => {
    await page.emulateMedia({ colorScheme });
    if (colorScheme === 'dark')
      await page.setViewportSize({ width: 390, height: 844 });
    const blueprint = await createEntityBlueprint(
      `email_${suffix()}`,
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
children = [{ type = "field", field = "contact", component = { id = "catalog.email_display", version = 1 } }]
[views.edit]
type = "stack"
children = [{ type = "field", field = "contact", component = { id = "catalog.email_edit", version = 1 } }]
`,
      },
    );
    await expect(
      createEntity(blueprint, [
        scalar('title', 'Invalid contact'),
        scalar('contact', 'not an email'),
      ]),
    ).rejects.toThrow(/422/);
    const entity = await createEntity(blueprint, [
      scalar('title', 'Contact'),
      scalar('contact', 'Name+tag@Example.com'),
    ]);
    await page.goto(`/entities/${entity.id}`);
    await expect(
      page.getByRole('link', { name: 'Name+tag@Example.com' }),
    ).toHaveAttribute('href', 'mailto:Name%2Btag@Example.com');
    await page.goto(`/entities/${entity.id}/edit`);
    const input = page.getByRole('textbox', { name: 'contact' });
    await expect(input).toHaveAttribute('type', 'email');
    await input.fill('invalid');
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(input).toHaveAttribute('aria-invalid', 'true');
    await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}/edit$`));
    await input.fill('Other+tag@Example.com');
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
    await page.reload();
    await expect(
      page.getByRole('link', { name: 'Other+tag@Example.com' }),
    ).toBeVisible();
    await page.goto(`/entities/${entity.id}/edit`);
    await input.clear();
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
    await expect(page.getByText('Not set', { exact: true })).toBeVisible();
  });
}

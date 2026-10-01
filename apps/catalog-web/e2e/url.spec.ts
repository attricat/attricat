import { expect, test } from '@playwright/test';
import { createEntity, createEntityBlueprint, scalar, suffix } from './helpers';

for (const mode of ['light', 'dark'] as const) {
  test(`URL editing and safe detail/table rendering (${mode})`, async ({
    page,
  }) => {
    await page.addInitScript(
      (value) => localStorage.setItem('attricat.color-mode', value),
      mode,
    );
    const code = `url_${suffix()}`;
    const blueprint = await createEntityBlueprint(
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
[views.edit]
type = "stack"
children = [{ type = "field", field = "website", component = { id = "catalog.url_edit", version = 1 } }]
[views.table]
type = "table"
columns = [{ field = "website", renderer = { id = "catalog.url_display", version = 1 } }]
`,
      },
    );
    const entity = await createEntity(blueprint, [
      scalar('title', 'URL example'),
      scalar('website', 'javascript:alert(1)'),
    ]);
    await page.goto(`/entities/${entity.id}`);
    await expect(
      page.getByText('javascript:alert(1)', { exact: true }),
    ).toBeVisible();
    await expect(page.locator('a[href^="javascript:"]')).toHaveCount(0);
    await page.goto(`/entities/${entity.id}/edit`);
    const input = page.getByRole('textbox', { name: 'website' });
    await expect(input).toHaveAttribute('type', 'url');
    await input.fill('https://example.com/a?query=1#section');
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
    const link = page.getByRole('link', { name: /https:\/\/example.com\/a/ });
    await expect(link).toHaveAttribute('target', '_blank');
    await expect(link).toHaveAttribute('rel', 'noopener noreferrer');
    await link.focus();
    await expect(link).toBeFocused();
    await page.goto(`/?blueprint=${code}`);
    await expect(
      page.getByRole('link', { name: /https:\/\/example.com\/a/ }),
    ).toBeVisible();
    await page.goto(`/entities/${entity.id}/edit`);
    await input.clear();
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
    await expect(
      page.getByRole('link', { name: /https:\/\/example.com\/a/ }),
    ).toHaveCount(0);
  });
}

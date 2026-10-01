import { expect, test } from '@playwright/test';
import { createEntityBlueprint, suffix } from './helpers';

for (const mode of ['light', 'dark'] as const) {
  test(`creates, edits, and clears a configured color in ${mode} mode`, async ({
    page,
  }, testInfo) => {
    await page.addInitScript(
      (value) => localStorage.setItem('attricat.color-mode', value),
      mode,
    );
    const code = `color_${suffix()}`;
    await createEntityBlueprint(
      code,
      'Color samples',
      `
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "hex"
value_type = "string"
`,
      {
        views: `
[views.detail]
type = "stack"
children = [{ type = "field", field = "title" }, { type = "field", field = "hex", component = { id = "catalog.color_display", version = 1 } }]
[views.edit]
type = "stack"
children = [{ type = "field", field = "title" }, { type = "field", field = "hex", component = { id = "catalog.color_edit", version = 1 } }]
[views.table]
type = "table"
columns = [{ field = "title" }, { field = "hex", renderer = { id = "catalog.color_display", version = 1 } }]
`,
      },
    );
    await page.goto('/entities/new');
    await page
      .getByRole('combobox', { name: 'Blueprint', exact: true })
      .click();
    await page.getByRole('option', { name: `Color samples (${code})` }).click();
    await page.getByRole('button', { name: 'Load blueprint' }).click();
    await page
      .getByRole('textbox', { name: 'title', exact: true })
      .fill('Color sample');
    const text = page.getByRole('textbox', { name: 'hex', exact: true });
    await expect(text).toHaveValue('');
    await text.fill('#fff');
    await page
      .getByRole('button', { name: 'Create entity', exact: true })
      .click();
    await expect(
      page.getByText('Enter a six-digit hex color, such as #1a2b3c.'),
    ).toBeVisible();
    await text.fill('#aBcDeF');
    await page.screenshot({
      path: testInfo.outputPath(`color-editor-${mode}.png`),
    });
    await text.press('Tab');
    await expect(page.getByLabel('Pick color for hex')).toBeFocused();
    await page
      .getByRole('button', { name: 'Create entity', exact: true })
      .click();
    await expect(page).toHaveURL(/\/entities\/[0-9a-f-]{36}$/);
    const entityUrl = page.url();
    const displayedColor = page.getByText('#aBcDeF', { exact: true });
    await expect(displayedColor).toBeVisible();
    await expect(
      displayedColor.locator('..').locator('[aria-hidden="true"]'),
    ).toHaveCSS('background-color', 'rgb(171, 205, 239)');
    await page.screenshot({
      path: testInfo.outputPath(`color-display-${mode}.png`),
    });
    await page.reload();
    await expect(page.getByText('#aBcDeF', { exact: true })).toBeVisible();
    await page.getByRole('link', { name: 'Edit entity' }).click();
    await page.getByLabel('Pick color for hex').fill('#ffffff');
    await expect(
      page.getByRole('textbox', { name: 'hex', exact: true }),
    ).toHaveValue('#ffffff');
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page.getByText('#ffffff', { exact: true })).toBeVisible();
    await page.goto(`/?blueprint=${code}`);
    await expect(page.getByText('#ffffff', { exact: true })).toBeVisible();
    await page.goto(entityUrl);
    await page.getByRole('link', { name: 'Edit entity' }).click();
    await page.getByRole('textbox', { name: 'hex', exact: true }).fill('');
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page.getByText('Not set', { exact: true })).toBeVisible();
    await expect(page.getByText('#ffffff', { exact: true })).toHaveCount(0);
  });
}

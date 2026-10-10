import { expect, test } from '@playwright/test';
import {
  commitField,
  createRecordBlueprint,
  recordSave,
  suffix,
} from './helpers';

for (const mode of ['light', 'dark'] as const) {
  test(`creates, edits, and clears a configured color in ${mode} mode`, async ({
    page,
  }, testInfo) => {
    await page.addInitScript(
      (value) => localStorage.setItem('attricat.color-mode', value),
      mode,
    );
    const code = `color_${suffix()}`;
    await createRecordBlueprint(
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
children = [{ type = "field", field = "title" }, { type = "field", field = "hex", component = { id = "attricat.color_display", version = 1 } }]
[views.table]
type = "table"
columns = [{ field = "title" }, { field = "hex", renderer = { id = "attricat.color_display", version = 1 } }]
`,
      },
    );
    await page.goto('/records/new');
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
      .getByRole('button', { name: 'Create record', exact: true })
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
      .getByRole('button', { name: 'Create record', exact: true })
      .click();
    await expect(page).toHaveURL(/\/records\/[0-9a-f-]{36}$/);
    const recordUrl = page.url();
    const recordId = recordUrl.split('/').at(-1)!;
    // The record page edits the value in place; the table displays it.
    const hex = page.getByRole('textbox', { name: 'hex', exact: true });
    await expect(hex).toHaveValue('#aBcDeF');
    await page.goto(`/?blueprint=${code}`);
    const displayedColor = page.getByText('#aBcDeF', { exact: true });
    await expect(displayedColor).toBeVisible();
    await expect(
      displayedColor.locator('..').locator('[aria-hidden="true"]'),
    ).toHaveCSS('background-color', 'rgb(171, 205, 239)');
    await page.screenshot({
      path: testInfo.outputPath(`color-display-${mode}.png`),
    });
    await page.goto(recordUrl);
    await expect(hex).toHaveValue('#aBcDeF');
    // Picking a color is a final choice and saves at once.
    const picked = recordSave(page, recordId);
    await page.getByLabel('Pick color for hex').fill('#ffffff');
    expect((await picked).ok()).toBe(true);
    await expect(hex).toHaveValue('#ffffff');
    await page.goto(`/?blueprint=${code}`);
    await expect(page.getByText('#ffffff', { exact: true })).toBeVisible();
    await page.goto(recordUrl);
    const saves: string[] = [];
    page.on('request', (request) => {
      if (request.method() === 'PUT') saves.push(request.url());
    });
    // An invalid color stays a local edit with its error and is not sent.
    await hex.fill('#fff');
    await hex.press('Enter');
    await expect(
      page.getByText('Enter a six-digit hex color, such as #1a2b3c.'),
    ).toBeVisible();
    await expect(hex).toHaveValue('#fff');
    expect(saves).toEqual([]);
    await hex.fill('');
    // Tab would move to the picker inside the same field, so commit with Enter.
    await commitField(page, recordId, hex, 'Enter');
    await page.reload();
    await expect(hex).toHaveValue('');
    await page.goto(`/?blueprint=${code}`);
    await expect(
      page.getByRole('link', { name: 'Color sample', exact: true }),
    ).toBeVisible();
    await expect(page.getByText('#ffffff', { exact: true })).toHaveCount(0);
  });
}

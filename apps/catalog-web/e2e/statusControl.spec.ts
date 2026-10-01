import { expect, test } from '@playwright/test';
import { createEntityBlueprint, suffix } from './helpers';

const statusSchema = JSON.stringify({
  type: 'string',
  enum: ['draft', 'live', 'done'],
  'x-attricat-status': {
    version: 1,
    options: [
      { code: 'draft', label: 'Draft' },
      { code: 'live', label: 'Live', tone: 'success' },
      { code: 'done', label: 'Done' },
    ],
    transitions: [
      { from: null, to: 'draft' },
      { from: 'draft', to: 'live' },
      { from: 'live', to: 'done' },
    ],
  },
});

for (const colorScheme of ['light', 'dark'] as const) {
  test(`creates, displays and edits status in ${colorScheme} mode; rejects stale saves`, async ({
    page,
    context,
  }) => {
    await page.emulateMedia({ colorScheme });
    const code = `status_${suffix()}`;
    await createEntityBlueprint(
      code,
      'Status items',
      `[[attributes]]\ncode = "title"\nvalue_type = "string"\n\n[[attributes]]\ncode = "status"\nvalue_type = "string"\nvalue_schema = '${statusSchema}'`,
    );
    await page.goto('/entities/new');
    await page.getByLabel('Blueprint').click();
    await page.getByRole('option', { name: `Status items (${code})` }).click();
    await page.getByRole('button', { name: 'Load blueprint' }).click();
    await page.getByLabel('title').fill('Status test');
    await page.getByRole('combobox', { name: 'status', exact: true }).click();
    await expect(
      page.getByRole('option', { name: 'Live', exact: true }),
    ).toBeDisabled();
    await page.getByRole('option', { name: 'Draft', exact: true }).click();
    await page.getByRole('button', { name: 'Create entity' }).click();
    await expect(page).toHaveURL(/\/entities\/[0-9a-f-]{36}$/);
    await expect(page.getByText('Draft', { exact: true })).toBeVisible();
    const editUrl = `${page.url()}/edit`;
    await page.goto(editUrl);
    const stalePage = await context.newPage();
    await stalePage.goto(editUrl);
    await expect(
      stalePage.getByRole('combobox', { name: 'status', exact: true }),
    ).toBeVisible();
    await page.getByRole('combobox', { name: 'status', exact: true }).click();
    await expect(
      page.getByRole('option', { name: 'Done', exact: true }),
    ).toBeDisabled();
    await page.getByRole('option', { name: 'Live', exact: true }).click();
    // A second unsaved selection must not use Live as the starting state.
    await page.getByRole('combobox', { name: 'status', exact: true }).click();
    await expect(
      page.getByRole('option', { name: 'Done', exact: true }),
    ).toBeDisabled();
    await page.keyboard.press('Escape');
    await page.getByRole('button', { name: 'Save changes' }).click();
    await expect(page).not.toHaveURL(/\/edit$/);
    await expect(page.getByText('Live', { exact: true })).toBeVisible();
    await stalePage
      .getByRole('combobox', { name: 'status', exact: true })
      .click();
    await stalePage.getByRole('option', { name: 'Live', exact: true }).click();
    await stalePage.getByRole('button', { name: 'Save changes' }).click();
    await expect(stalePage.getByRole('alert')).toContainText(
      'entity changed since it was loaded',
    );
    await expect(
      stalePage.getByRole('combobox', { name: 'status', exact: true }),
    ).toContainText('Live');
    await stalePage.close();
  });
}

import { expect, test } from '@playwright/test';
import { createEntityBlueprint, entitySave, suffix } from './helpers';

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
  test(`creates, displays and edits status in ${colorScheme} mode; flags conflicting saves`, async ({
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
    const entityUrl = page.url();
    const entityId = entityUrl.split('/').at(-1)!;
    const stalePage = await context.newPage();
    await stalePage.goto(entityUrl);
    await expect(
      stalePage.getByRole('combobox', { name: 'status', exact: true }),
    ).toBeVisible();
    const status = page.getByRole('combobox', { name: 'status', exact: true });
    await status.click();
    await expect(
      page.getByRole('option', { name: 'Done', exact: true }),
    ).toBeDisabled();
    // A status choice saves at once and becomes the next starting state.
    const saved = entitySave(page, entityId);
    await page.getByRole('option', { name: 'Live', exact: true }).click();
    expect((await saved).ok()).toBe(true);
    await expect(status).toContainText('Live');
    await status.click();
    await expect(
      page.getByRole('option', { name: 'Done', exact: true }),
    ).toBeEnabled();
    await expect(
      page.getByRole('option', { name: 'Draft', exact: true }),
    ).toBeDisabled();
    await page.keyboard.press('Escape');
    await page.reload();
    await expect(status).toContainText('Live');

    // The other page still edits the version it loaded.
    const staleStatus = stalePage.getByRole('combobox', {
      name: 'status',
      exact: true,
    });
    const staleSave = entitySave(stalePage, entityId);
    await staleStatus.click();
    await stalePage.getByRole('option', { name: 'Live', exact: true }).click();
    expect((await staleSave).status()).toBe(409);
    const conflict = stalePage
      .getByRole('alert')
      .filter({ hasText: 'Someone else changed this entity' });
    await expect(conflict).toBeVisible();
    await expect(
      conflict.getByRole('button', { name: 'Keep my changes' }),
    ).toBeVisible();
    await conflict.getByRole('button', { name: 'Use latest values' }).click();
    await expect(conflict).toBeHidden();
    await expect(staleStatus).toContainText('Live');
    await stalePage.close();
  });
}

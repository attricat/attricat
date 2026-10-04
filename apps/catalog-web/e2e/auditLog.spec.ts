import { expect, test } from '@playwright/test';
import { createEntity, createEntityBlueprint, scalar, suffix } from './helpers';

test('records a browser edit and filters the audit log', async ({ page }) => {
  const blueprint = await createEntityBlueprint(
    `audited_${suffix()}`,
    'Audited product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const entity = await createEntity(blueprint, [scalar('title', 'Before')]);

  await page.goto(`/entities/${entity.id}/edit`);
  await page.getByLabel('title').fill('After audit');
  await page.getByRole('button', { name: 'Save changes' }).click();
  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));

  await page.goto('/manage/audit-log');
  const update = page
    .getByRole('row')
    .filter({ hasText: `id: ${entity.id}, type: entity` });
  await expect(update).toContainText('owner@example.test');
  await expect(update).toContainText('catalog.v1.entities.entity_id.update');
  await expect(update).toContainText('success');

  await update.getByRole('button', { name: /^View details for/ }).click();
  // The MUI drawer has no dialog role, so locate its panel by its heading.
  const drawer = page
    .locator('.MuiDrawer-paper')
    .filter({ has: page.getByRole('heading', { name: 'Audit event' }) });
  await expect(drawer).toContainText(entity.id);
  await expect(drawer).toContainText('Request ID');
  await page.keyboard.press('Escape');
  await expect(drawer).toBeHidden();

  await page.getByLabel('Target type').fill('blueprint');
  await page.getByRole('button', { name: 'Apply filters' }).click();
  await expect(update).toBeHidden();
  await expect(
    page.getByRole('row').filter({ hasText: 'type: blueprint' }).first(),
  ).toBeVisible();
  await expect(
    page.getByRole('row').filter({ hasText: 'type: entity' }),
  ).toHaveCount(0);
});

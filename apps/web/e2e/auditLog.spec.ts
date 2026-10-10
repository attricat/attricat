import { expect, test } from '@playwright/test';
import {
  commitField,
  createRecord,
  createRecordBlueprint,
  scalar,
  suffix,
} from './helpers';

test('records a browser edit and filters the audit log', async ({ page }) => {
  const blueprint = await createRecordBlueprint(
    `audited_${suffix()}`,
    'Audited product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const record = await createRecord(blueprint, [scalar('title', 'Before')]);

  await page.goto(`/records/${record.id}`);
  const title = page.getByLabel('title');
  await title.fill('After audit');
  await commitField(page, record.id, title);

  await page.goto('/manage/audit-log');
  const update = page
    .getByRole('row')
    .filter({ hasText: `id: ${record.id}, type: record` });
  await expect(update).toContainText('owner@example.test');
  await expect(update).toContainText('attricat.v1.records.record_id.update');
  await expect(update).toContainText('success');

  await update.getByRole('button', { name: /^View details for/ }).click();
  const drawer = page.getByRole('dialog', { name: 'Audit event' });
  await expect(drawer).toContainText(record.id);
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
    page.getByRole('row').filter({ hasText: 'type: record' }),
  ).toHaveCount(0);
});

import { expect, test, type Page } from '@playwright/test';
import {
  appendValues,
  createContext,
  createRecord,
  createRecordBlueprint,
  defaultContext,
  request,
  scalar,
  suffix,
} from './helpers';

const openRecordInContext = async (
  page: Page,
  recordId: string,
  contextCode: string,
) => {
  await page.goto(`/records/${recordId}`);
  await page.getByRole('tab', { name: contextCode }).click();
};

const publishFromMenu = async (page: Page) => {
  await page.getByRole('button', { name: 'Publication', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Publish', exact: true }).click();
};

test('enables an export channel, publishes to it and enforces its checks', async ({
  page,
}) => {
  const code = `exported_${suffix()}`;
  const ruleCode = `summary-required-${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Exported product',
    `[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "summary"
value_type = "string"

[[rules]]
code = "${ruleCode}"
name = "Summary required ${code}"
severity = "error"

[[rules.triggers]]
type = "manual"

[rules.predicate]
type = "required"
attribute_code = "summary"`,
  );
  const root = await defaultContext();
  const channel = await createContext(`export_${suffix()}`, root.id);
  const record = await createRecord(blueprint, [
    scalar('title', 'Exported record'),
  ]);
  // Channels may only require enabled rules.
  const rules =
    await request<Array<{ id: string; code: string; version: number }>>(
      '/rules',
    );
  const rule = rules.find(({ code }) => code === ruleCode)!;
  await request(`/rules/${rule.id}/versions/${rule.version}/enable`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: '{}',
  });

  await openRecordInContext(page, record.id, channel.code);
  await expect(page.getByText('Not an export channel')).toBeVisible();

  await page.goto('/manage/exports');
  const channelRow = page.getByRole('row').filter({ hasText: channel.code });
  await expect(channelRow).toContainText('Disabled');
  await channelRow.getByRole('switch').first().click();
  await expect(channelRow).toContainText('Enabled');
  await page.reload();
  await expect(channelRow).toContainText('Enabled');

  await openRecordInContext(page, record.id, channel.code);
  await expect(page.getByText('Not published', { exact: true })).toBeVisible();
  await publishFromMenu(page);
  await expect(page.getByText('Published', { exact: true })).toBeVisible();

  await page.goto('/manage/exports');
  await channelRow.getByLabel('Required rules').fill(ruleCode);
  await page.keyboard.press('Enter');
  await expect(
    channelRow.getByRole('button', { name: ruleCode }),
  ).toBeVisible();
  await page.reload();
  await expect(
    channelRow.getByRole('button', { name: ruleCode }),
  ).toBeVisible();

  await openRecordInContext(page, record.id, channel.code);
  await expect(page.getByText('Not ready', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Publication', exact: true }).click();
  await expect(
    page.getByRole('menuitem', { name: /^Republish/ }),
  ).toBeDisabled();
  await page.keyboard.press('Escape');
  // The disabled menu item is a hint; the API enforces the channel's checks.
  await expect(
    request(`/v1/records/${record.id}/publications`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ context_id: channel.id }),
    }),
  ).rejects.toThrow(/422 .*publication_checks_failed/);

  await appendValues(record.id, [
    { ...scalar('summary', 'Now complete'), context_id: root.id },
  ]);
  await openRecordInContext(page, record.id, channel.code);
  await expect(page.getByText('Not published', { exact: true })).toBeVisible();
  await expect(page.getByText('Not ready', { exact: true })).toBeHidden();
  await publishFromMenu(page);
  await expect(page.getByText('Published', { exact: true })).toBeVisible();

  await page.goto('/manage/exports');
  await channelRow.getByRole('switch').first().click();
  await expect(channelRow).toContainText('Disabled');
  await openRecordInContext(page, record.id, channel.code);
  await expect(page.getByText('Not an export channel')).toBeVisible();
});

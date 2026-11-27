import { expect, test } from '@playwright/test';
import { request, suffix } from './helpers';

test('adds, replaces and deletes a workspace extension secret', async ({
  page,
}) => {
  const name = `e2e-token-${suffix()}`;
  await page.goto('/manage/extensions/secrets');
  await expect(
    page.getByRole('tab', { name: 'Secrets', selected: true }),
  ).toBeVisible();

  await page.getByRole('button', { name: 'Add secret' }).click();
  const addDialog = page.getByRole('dialog', { name: 'Add secret' });
  await addDialog.getByRole('textbox', { name: 'Name' }).fill(name);
  await addDialog.getByLabel('Value').fill('first-value');
  await addDialog.getByRole('button', { name: 'Add secret' }).click();
  await expect(addDialog).toBeHidden();

  const table = page.getByRole('table', { name: 'Secrets' });
  await expect(table.getByText(name, { exact: true })).toBeVisible();
  // The value never comes back from the server.
  await expect(page.getByText('first-value')).toHaveCount(0);
  const listed = await request<Array<Record<string, unknown>>>(
    '/workspace/extension-secrets',
  );
  expect(listed.find((secret) => secret.name === name)).not.toHaveProperty(
    'value',
  );

  await page.getByRole('button', { name: `Replace value of ${name}` }).click();
  const replaceDialog = page.getByRole('dialog', {
    name: `Replace value of ${name}`,
  });
  await replaceDialog.getByLabel('New value').fill('second-value');
  await replaceDialog.getByRole('button', { name: 'Replace value' }).click();
  await expect(replaceDialog).toBeHidden();
  await expect(page.getByText(`Value of ${name} replaced.`)).toBeVisible();

  await page.getByRole('button', { name: `Delete ${name}` }).click();
  const deleteDialog = page.getByRole('dialog', {
    name: `Delete secret ${name}?`,
  });
  await deleteDialog.getByRole('button', { name: 'Delete secret' }).click();
  await expect(deleteDialog).toBeHidden();
  await expect(table.getByText(name, { exact: true })).toHaveCount(0);
});

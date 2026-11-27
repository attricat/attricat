import { readFileSync } from 'node:fs';
import { expect, test } from '@playwright/test';
import { request, suffix } from './helpers';

/** The example extension, which requests `secrets.read`. */
const extensionId = 'attricat-extension-example';

test('adds, replaces and deletes a secret of one extension', async ({
  page,
}) => {
  const archive = process.env.ATTRICAT_E2E_EXAMPLE_EXTENSION_ARCHIVE;
  if (!archive) throw new Error('E2E example extension archive is unavailable');
  // The extension spec may have installed it already. Otherwise it is
  // installed only for this test and removed again, so that spec still
  // side-loads it fresh.
  const installed =
    await request<Array<{ extension_id: string }>>('/extensions');
  const installedHere = !installed.some(
    (extension) => extension.extension_id === extensionId,
  );
  if (installedHere)
    await request('/extensions/sideload', {
      method: 'POST',
      headers: { 'Content-Type': 'application/zstd' },
      body: readFileSync(archive),
    });
  try {
    const name = `e2e-token-${suffix()}`;
    await page.goto(`/manage/extensions/${extensionId}?tab=secrets`);
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
      `/extensions/${extensionId}/secrets`,
    );
    expect(listed.find((secret) => secret.name === name)).toMatchObject({
      set: true,
      declared: false,
    });
    expect(JSON.stringify(listed)).not.toContain('first-value');

    await page
      .getByRole('button', { name: `Replace value of ${name}` })
      .click();
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
  } finally {
    if (installedHere)
      await request(`/extensions/${extensionId}`, { method: 'DELETE' });
  }
});

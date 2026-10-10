import { expect, test, type Page } from '@playwright/test';
import { request, suffix } from './helpers';

const draftKeyPrefix = 'attricat.draft.';
const restoreDialogName = 'Restore unsaved draft?';

const storedDraftValues = (page: Page) =>
  page.evaluate(
    (prefix) =>
      Object.keys(sessionStorage)
        .filter((key) => key.startsWith(prefix))
        .map((key) => JSON.parse(sessionStorage.getItem(key) ?? '{}').value),
    draftKeyPrefix,
  );

// The unload warning is expected while a draft is unsaved.
const reload = async (page: Page) => {
  page.once('dialog', (dialog) => void dialog.accept());
  await page.reload();
};

const restoreDialog = (page: Page) =>
  page.getByRole('dialog', { name: restoreDialogName });

test('offers a reusable attribute draft over a newer revision', async ({
  page,
}) => {
  const code = `draft_attribute_${suffix()}`;
  const definition = `code = "${code}"\nname = "Draft attribute"\nvalue_type = "string"\n`;
  const attribute = await request<{ definition_id: string }>(
    '/reusable-attributes',
    {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ definition }),
    },
  );

  await page.goto(`/manage/reusable-attributes/${attribute.definition_id}`);
  const editor = page.locator('.monaco-editor').first();
  await editor.click();
  await page.keyboard.press('ControlOrMeta+End');
  await page.keyboard.insertText('\n#draft_note');
  await expect
    .poll(() => storedDraftValues(page))
    .toContainEqual(expect.stringContaining('#draft_note'));

  await request(`/reusable-attributes/${attribute.definition_id}/versions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      definition: definition.replace('Draft attribute', 'Server attribute'),
    }),
  });
  await reload(page);
  await expect(restoreDialog(page)).toContainText(
    'The source has changed since this draft was saved.',
  );
  await expect(editor).toContainText('Server attribute');
  await expect(editor).not.toContainText('#draft_note');
  await restoreDialog(page)
    .getByRole('button', { name: 'Restore draft' })
    .click();
  await expect(editor).toContainText('#draft_note');
  await expect(editor).toContainText('Draft attribute');
});

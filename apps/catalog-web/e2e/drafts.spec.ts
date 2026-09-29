import { expect, test, type Page } from '@playwright/test';
import {
  createContext,
  createEntity,
  createEntityBlueprint,
  defaultContext,
  request,
  scalar,
  suffix,
  type Entity,
} from './helpers';

const draftKeyPrefix = 'catalog.draft.';
const restoreDialogName = 'Restore unsaved draft?';

const createTitledEntity = async (title = 'Original title') => {
  const blueprint = await createEntityBlueprint(
    `draft_product_${suffix()}`,
    'Draft products',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  return createEntity(blueprint, [scalar('title', title)]);
};

const storedDraftValues = (page: Page) =>
  page.evaluate(
    (prefix) =>
      Object.keys(sessionStorage)
        .filter((key) => key.startsWith(prefix))
        .map((key) => JSON.parse(sessionStorage.getItem(key) ?? '{}').value),
    draftKeyPrefix,
  );

const editTitle = async (page: Page, entity: Entity, title: string) => {
  await page.goto(`/entities/${entity.id}/edit`);
  await page.getByLabel('title').fill(title);
  await expect
    .poll(() => storedDraftValues(page))
    .toContainEqual(expect.objectContaining({ title }));
};

// The unload warning is expected while a draft is unsaved.
const reload = async (page: Page) => {
  page.once('dialog', (dialog) => void dialog.accept());
  await page.reload();
};

const restoreDialog = (page: Page) =>
  page.getByRole('dialog', { name: restoreDialogName });

test('restores an entity draft after refresh only when confirmed', async ({
  page,
}) => {
  const entity = await createTitledEntity();
  await editTitle(page, entity, 'Draft title');

  await reload(page);
  await expect(restoreDialog(page)).toBeVisible();
  await expect(restoreDialog(page)).not.toContainText('source has changed');
  await expect(page.getByLabel('title')).toHaveValue('Original title');
  await restoreDialog(page)
    .getByRole('button', { name: 'Restore draft' })
    .click();
  await expect(page.getByLabel('title')).toHaveValue('Draft title');

  await page.getByRole('button', { name: 'Save changes' }).click();
  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
  expect(await storedDraftValues(page)).toEqual([]);
  await page.goto(`/entities/${entity.id}/edit`);
  await expect(page.getByLabel('title')).toHaveValue('Draft title');
  await expect(restoreDialog(page)).toHaveCount(0);
});

test('warns about a changed source and discards without editing the form', async ({
  page,
}) => {
  const entity = await createTitledEntity();
  await editTitle(page, entity, 'Draft title');
  await request(`/v1/entities/${entity.id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      values: [
        {
          ...scalar('title', 'Server title'),
          context_id: (await defaultContext()).id,
        },
      ],
      relationships: [],
    }),
  });

  await reload(page);
  await expect(restoreDialog(page)).toContainText(
    'The source has changed since this draft was saved.',
  );
  await restoreDialog(page)
    .getByRole('button', { name: 'Discard draft' })
    .click();
  await expect(page.getByLabel('title')).toHaveValue('Server title');
  expect(await storedDraftValues(page)).toEqual([]);

  await page.reload();
  await expect(page.getByLabel('title')).toHaveValue('Server title');
  await expect(restoreDialog(page)).toHaveCount(0);
});

test('keeps entity drafts separate per context', async ({ page }) => {
  const entity = await createTitledEntity();
  const context = await createContext(
    `draft_market_${suffix()}`,
    (await defaultContext()).id,
  );
  await editTitle(page, entity, 'Default draft');

  const contextTab = page.getByRole('tab', { name: context.code });
  await contextTab.click();
  await expect(contextTab).toHaveAttribute('aria-selected', 'true');
  await expect(restoreDialog(page)).toHaveCount(0);
  await expect(page.getByLabel('title')).not.toHaveValue('Default draft');

  await page.getByRole('tab', { name: 'Default' }).click();
  await expect(restoreDialog(page)).toBeVisible();
  await restoreDialog(page)
    .getByRole('button', { name: 'Restore draft' })
    .click();
  await expect(page.getByLabel('title')).toHaveValue('Default draft');
});

test('retains the draft when saving fails', async ({ page }) => {
  const entity = await createTitledEntity();
  await page.route(`**/api/v1/entities/${entity.id}`, (route) =>
    route.request().method() === 'PUT'
      ? route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ error: 'Simulated failure' }),
        })
      : route.fallback(),
  );
  await editTitle(page, entity, 'Unsaved title');
  await page.getByRole('button', { name: 'Save changes' }).click();
  await expect(page.getByRole('alert')).toBeVisible();
  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}/edit$`));

  await reload(page);
  await expect(restoreDialog(page)).toBeVisible();
});

test('keeps editing when session storage is unavailable', async ({ page }) => {
  await page.addInitScript(() => {
    const setItem = Storage.prototype.setItem;
    Storage.prototype.setItem = function (key: string, value: string) {
      if (key.startsWith('catalog.draft.'))
        throw new DOMException('Quota exceeded', 'QuotaExceededError');
      return setItem.call(this, key, value);
    };
  });
  const entity = await createTitledEntity();
  await page.goto(`/entities/${entity.id}/edit`);
  await page.getByLabel('title').fill('Saved without drafts');
  await page.getByRole('button', { name: 'Save changes' }).click();
  await expect(page).toHaveURL(new RegExp(`/entities/${entity.id}$`));
  await expect(page.getByText('Saved without drafts')).toBeVisible();
});

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

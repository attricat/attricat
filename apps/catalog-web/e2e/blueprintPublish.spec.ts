import { expect, test, type Page } from '@playwright/test';
import { replaceDefinition, suffix } from './helpers';

const definition = (code: string, extraAttributes = '') => `format_version = 1
code = "${code}"
name = "Authored product"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"
${extraAttributes}`;

const publishDraft = async (page: Page, version: number) => {
  await expect(page.getByText(`Latest: v${version}`)).toBeVisible();
  await expect(page.getByText('Draft', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Publish', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Publish blueprint?' });
  await dialog.getByRole('button', { name: 'Publish', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText('Published', { exact: true })).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Publish', exact: true }),
  ).toBeHidden();
};

test('authors, publishes, revises and migrates a blueprint in the browser', async ({
  page,
}) => {
  const code = `authored_${suffix()}`;

  await page.goto('/manage/blueprints/new');
  await page.getByRole('button', { name: 'Dismiss', exact: true }).click();
  await replaceDefinition(page, definition(code));
  await page.getByRole('button', { name: 'Save draft' }).click();

  await expect(page).toHaveURL(/\/manage\/blueprints\/[0-9a-f-]{36}$/);
  const blueprintUrl = page.url();
  await expect(
    page.getByRole('heading', { name: 'Authored product' }),
  ).toBeVisible();
  await publishDraft(page, 1);

  await page.goto('/entities/new');
  await page.getByLabel('Blueprint').click();
  await page
    .getByRole('option', { name: `Authored product (${code})` })
    .click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
  await page.getByLabel('title').fill('Authored entity');
  await page.getByRole('button', { name: 'Create entity' }).click();
  await expect(page).toHaveURL(/\/entities\/[0-9a-f-]{36}$/);
  const entityUrl = page.url();
  await expect(page.getByText('Authored entity')).toBeVisible();

  await page.goto(blueprintUrl);
  await page.getByRole('button', { name: 'Edit blueprint' }).click();
  await expect(
    page.getByRole('heading', { name: 'New blueprint revision' }),
  ).toBeVisible();
  await replaceDefinition(
    page,
    definition(
      code,
      '\n[[attributes]]\ncode = "description"\nvalue_type = "string"\n',
    ),
  );
  await page.getByRole('button', { name: 'Save draft' }).click();
  await expect(page).toHaveURL(blueprintUrl);
  await publishDraft(page, 2);

  await page
    .getByRole('button', { name: 'Migrate compatible entities' })
    .click();
  const migration = page.getByRole('dialog', {
    name: 'Migrate compatible entities',
  });
  await migration.getByRole('button', { name: 'Start migration' }).click();
  await expect(migration).toBeHidden();
  const batches = page.getByRole('table', { name: 'Entity migration batches' });
  await expect(async () => {
    await page.getByRole('button', { name: 'Refresh', exact: true }).click();
    await expect(batches.getByText('Completed')).toBeVisible({
      timeout: 1_000,
    });
  }).toPass();
  await expect(batches.getByText('1 of 1')).toBeVisible();

  await page.goto(entityUrl);
  await expect(page.getByLabel('Matches current schema')).toBeVisible();
  await expect(page.getByText('Authored entity')).toBeVisible();
});

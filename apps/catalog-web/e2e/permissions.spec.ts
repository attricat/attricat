import { expect, test, type Locator, type Page } from '@playwright/test';
import {
  commitField,
  createBlueprint,
  createEntity,
  createEntityBlueprint,
  replaceDefinition,
  request,
  scalar,
  signInAsMember,
  suffix,
  type Context,
} from './helpers';

const refusal = (scope: Page | Locator) =>
  scope
    .getByRole('alert')
    .filter({ hasText: 'you are not authorized to perform this action' });

const draftDefinition = (code: string) => `format_version = 1
code = "${code}"
name = "Restricted draft"
kind = "entity"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]

[[attributes]]
code = "title"
value_type = "string"`;

const latestStatus = async (blueprintId: string) => {
  const [latest] = await request<Array<{ status: string }>>(
    `/blueprints/${blueprintId}/versions`,
  );
  return latest.status;
};

const attemptPublish = async (page: Page) => {
  await page.getByRole('button', { name: 'Publish', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Publish blueprint?' });
  await dialog.getByRole('button', { name: 'Publish', exact: true }).click();
  await expect(refusal(dialog)).toBeVisible();
  await dialog.getByRole('button', { name: 'Cancel' }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByText('Draft', { exact: true })).toBeVisible();
};

test('a viewer can browse the catalog but every write is refused', async ({
  browser,
}) => {
  const code = `viewer_${suffix()}`;
  const contextCode = `viewer_ctx_${suffix()}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Viewer product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const entity = await createEntity(blueprint, [
    scalar('title', 'Readable product'),
  ]);
  const draft = await createBlueprint(
    draftDefinition(`viewer_draft_${suffix()}`),
  );
  const page = await signInAsMember(browser, 'viewer');

  await expect(
    page.getByRole('link', { name: 'Workspace management' }),
  ).toBeHidden();
  await page.goto('/manage/workspace/members');
  await expect(page.getByRole('alert')).toHaveText(
    'You are not authorized to manage members.',
  );

  await page.goto(`/entities/${entity.id}`);
  await expect(
    page.getByRole('heading', { level: 1, name: 'Readable product' }),
  ).toBeVisible();
  // Without write access the entity page shows values, not editable fields.
  await expect(
    page.getByRole('paragraph').filter({ hasText: 'Readable product' }),
  ).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'title' })).toHaveCount(0);
  await expect(
    page.getByRole('button', {
      name: 'Add custom attribute or attribute group',
    }),
  ).toHaveCount(0);

  await page.goto('/entities/new');
  await page.getByLabel('Blueprint').click();
  await page.getByRole('option', { name: `Viewer product (${code})` }).click();
  await page.getByRole('button', { name: 'Load blueprint' }).click();
  await page.getByLabel('title').fill('Unauthorized product');
  await page.getByRole('button', { name: 'Create record' }).click();
  await expect(refusal(page)).toBeVisible();
  await expect(page).toHaveURL(/\/entities\/new(\?|$)/);

  await page.goto(`/manage/blueprints/${draft.blueprint.id}`);
  await attemptPublish(page);
  expect(await latestStatus(draft.blueprint.id)).toBe('draft');

  await page.goto('/manage/contexts/new');
  await page.getByLabel('Code').fill(contextCode);
  await page.getByLabel('Parent context').click();
  await page.getByRole('option', { name: 'default' }).click();
  await page.getByRole('button', { name: 'Create context' }).click();
  await expect(refusal(page)).toBeVisible();
  const contexts = await request<Context[]>('/contexts');
  expect(contexts.map(({ code }) => code)).not.toContain(contextCode);

  await page.context().close();
});

test('an editor saves blueprint drafts and entities but cannot publish', async ({
  browser,
}) => {
  const code = `editor_${suffix()}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Editor product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  const entity = await createEntity(blueprint, [
    scalar('title', 'Original title'),
  ]);
  const page = await signInAsMember(browser, 'editor');

  await page.goto(`/entities/${entity.id}`);
  const title = page.getByLabel('title');
  await title.fill('Edited by editor');
  await commitField(page, entity.id, title);
  await page.reload();
  await expect(page.getByLabel('title')).toHaveValue('Edited by editor');

  await page.goto('/manage/blueprints/new');
  await page.getByRole('button', { name: 'Dismiss', exact: true }).click();
  await replaceDefinition(page, draftDefinition(`editor_draft_${suffix()}`));
  await page.getByRole('button', { name: 'Save draft' }).click();
  await expect(page).toHaveURL(/\/manage\/blueprints\/[0-9a-f-]{36}$/);
  const blueprintId = new URL(page.url()).pathname.split('/').at(-1)!;
  await expect(
    page.getByRole('heading', { name: 'Restricted draft' }),
  ).toBeVisible();

  await attemptPublish(page);
  expect(await latestStatus(blueprintId)).toBe('draft');

  await page.context().close();
});

import { expect, test, type Page } from '@playwright/test';
import {
  commitField,
  createBlueprint,
  createRecord,
  createRecordBlueprint,
  defaultContext,
  replaceDefinition,
  request,
  scalar,
  signInAsMember,
  suffix,
  type Context,
} from './helpers';

const draftDefinition = (code: string) => `format_version: 1
code: '${code}'
name: Restricted draft
kind: record
views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string`;

const latestStatus = async (blueprintId: string) => {
  const [latest] = await request<Array<{ status: string }>>(
    `/blueprints/${blueprintId}/versions`,
  );
  return latest.status;
};

// The UI hides refused actions; the API must still refuse them itself.
const expectRefusedWrite = async (page: Page, path: string, data?: object) => {
  const csrf = (await page.context().cookies()).find(
    ({ name }) => name === 'attricat_csrf',
  );
  if (!csrf) throw new Error('The member session has no CSRF cookie');
  const response = await page.request.post(`/api${path}`, {
    data,
    headers: { 'X-Attricat-Csrf': csrf.value },
  });
  expect(response.status()).toBe(403);
};

// Without the permission the action is not offered at all.
const expectPublishHidden = async (page: Page) => {
  await expect(page.getByText('Draft', { exact: true })).toBeVisible();
  await expect(
    page.getByRole('button', { name: 'Publish', exact: true }),
  ).toHaveCount(0);
};

test('a viewer can browse the catalog but every write is refused', async ({
  browser,
}) => {
  const code = `viewer_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Viewer product',
    'attributes:\n  - code: title\n    value_type: string',
  );
  const record = await createRecord(blueprint, [
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

  await page.goto(`/records/${record.id}`);
  await expect(
    page.getByRole('heading', { level: 1, name: 'Readable product' }),
  ).toBeVisible();
  // Without write access the record page shows values, not editable fields.
  await expect(
    page.getByRole('paragraph').filter({ hasText: 'Readable product' }),
  ).toBeVisible();
  await expect(page.getByRole('textbox', { name: 'title' })).toHaveCount(0);
  await expect(
    page.getByRole('button', {
      name: 'Add custom attribute or attribute group',
    }),
  ).toHaveCount(0);

  // Pages for writes the session may not make refuse before any input.
  await page.goto('/records/new');
  await expect(page.getByRole('alert')).toHaveText(
    'You are not authorized to create records in this workspace.',
  );
  await expect(page.getByRole('combobox', { name: 'Blueprint' })).toHaveCount(
    0,
  );

  const context = await defaultContext();
  await expectRefusedWrite(page, '/v1/records', {
    blueprint: { code, version: blueprint.blueprint.version },
    values: [
      { ...scalar('title', 'Unauthorized product'), context_id: context.id },
    ],
  });

  await page.goto(`/manage/blueprints/${draft.blueprint.id}`);
  await expectPublishHidden(page);
  await expectRefusedWrite(
    page,
    `/blueprints/${draft.blueprint.id}/versions/${draft.blueprint.version}/publish`,
  );
  expect(await latestStatus(draft.blueprint.id)).toBe('draft');

  await page.goto('/manage/contexts/new');
  await expect(page.getByRole('alert')).toHaveText(
    'You are not authorized to create contexts in this workspace.',
  );
  const contextCode = `viewer_ctx_${suffix()}`;
  await expectRefusedWrite(page, '/contexts', {
    code: contextCode,
    parent_id: context.id,
    data: {},
  });
  const contexts = await request<Context[]>('/contexts');
  expect(contexts.map(({ code }) => code)).not.toContain(contextCode);

  await page.context().close();
});

test('an editor saves blueprint drafts and records but cannot publish', async ({
  browser,
}) => {
  const code = `editor_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Editor product',
    'attributes:\n  - code: title\n    value_type: string',
  );
  const record = await createRecord(blueprint, [
    scalar('title', 'Original title'),
  ]);
  const page = await signInAsMember(browser, 'editor');

  await page.goto(`/records/${record.id}`);
  const title = page.getByLabel('title');
  await title.fill('Edited by editor');
  await commitField(page, record.id, title);
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

  await expectPublishHidden(page);
  const [latest] = await request<Array<{ version: number }>>(
    `/blueprints/${blueprintId}/versions`,
  );
  await expectRefusedWrite(
    page,
    `/blueprints/${blueprintId}/versions/${latest.version}/publish`,
  );
  expect(await latestStatus(blueprintId)).toBe('draft');

  await page.context().close();
});

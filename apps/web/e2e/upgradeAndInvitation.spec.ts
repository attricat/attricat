import { expect, test } from '@playwright/test';
import {
  createContext,
  createMember,
  createRecord,
  createRecordBlueprint,
  createRevision,
  defaultContext,
  fieldByLabel,
  publishRevision,
  request,
  scalar,
  suffix,
} from './helpers';
import { e2eMailpitUrl } from './ports.ts';

const png = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  'base64',
);

const definition = (
  code: string,
  attributes: string,
  extra = '',
) => `format_version: 1
code: '${code}'
name: Upgraded product
kind: record
${extra}views:
  dropdown_option:
    type: dropdown_option
    fields: [title]
attributes:
  - code: title
    value_type: string
${attributes}`;

const upgrade = async (blueprintId: string, revision: string) =>
  publishRevision(await createRevision(blueprintId, revision));

test('an upgrade asks about a value in another context', async ({ page }) => {
  const code = `context_upgrade_${suffix()}`;
  const contextCode = `market-${suffix()}`;
  const weight =
    '  - code: weight\n    value_type: string\n    context_editable: all';
  const blueprint = await createRecordBlueprint(
    code,
    'Upgraded product',
    `attributes:\n  - code: title\n    value_type: string\n${weight}`,
  );
  const fallback = await defaultContext();
  const market = await createContext(contextCode, fallback.id);
  const record = await createRecord(blueprint, [scalar('title', 'Scale')]);
  await request(`/records/${record.id}/values`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      values: [{ ...scalar('weight', 'ok. 2 kg'), context_id: market.id }],
    }),
  });
  await upgrade(
    blueprint.blueprint.id,
    definition(
      code,
      '  - code: weight\n    value_type: integer\n    context_editable: all',
    ),
  );

  await page.goto(`/records/${record.id}/migrate`);
  await expect(page.getByText(`weight in ${contextCode}`)).toBeVisible();
  await expect(page.getByText('Previous value: ok. 2 kg')).toBeVisible();
  // Unanswered, it blocks the upgrade rather than losing the value.
  await page.getByRole('button', { name: 'Upgrade record' }).click();
  await expect(page).toHaveURL(new RegExp(`/records/${record.id}/migrate$`));
  await page
    .getByRole('textbox', { name: `Replacement in ${contextCode}` })
    .fill('2');
  await page.getByRole('button', { name: 'Upgrade record' }).click();
  await expect(page).toHaveURL(new RegExp(`/records/${record.id}$`));

  const { values } = await request<{
    values: Array<{
      attribute_code: string;
      context_id: string;
      value: unknown;
    }>;
  }>(`/v1/records/${record.id}`);
  expect(
    values.find(
      ({ attribute_code, context_id }) =>
        attribute_code === 'weight' && context_id === market.id,
    )?.value,
  ).toBe(2);
});

test('an upgrade clears a value the user emptied', async ({ page }) => {
  const code = `clear_upgrade_${suffix()}`;
  const note = '  - code: note\n    value_type: string';
  const blueprint = await createRecordBlueprint(
    code,
    'Upgraded product',
    `attributes:\n  - code: title\n    value_type: string\n${note}`,
  );
  const record = await createRecord(blueprint, [
    scalar('title', 'Kept'),
    scalar('note', 'Clear me'),
  ]);
  await upgrade(
    blueprint.blueprint.id,
    definition(code, `${note}\n  - code: added\n    value_type: string`),
  );

  await page.goto(`/records/${record.id}/migrate`);
  await fieldByLabel(page, 'note').fill('');
  await page.getByRole('button', { name: 'Upgrade record' }).click();
  await expect(page).toHaveURL(new RegExp(`/records/${record.id}$`));

  const { values } = await request<{
    values: Array<{ attribute_code: string }>;
  }>(`/v1/records/${record.id}`);
  expect(values.map(({ attribute_code }) => attribute_code)).not.toContain(
    'note',
  );
  expect(values.map(({ attribute_code }) => attribute_code)).toContain('title');
});

test('an upgrade takes a newly required file', async ({ page }) => {
  const code = `file_upgrade_${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Upgraded product',
    'attributes:\n  - code: title\n    value_type: string',
  );
  const record = await createRecord(blueprint, [scalar('title', 'Manual')]);
  await upgrade(
    blueprint.blueprint.id,
    definition(
      code,
      '  - code: manual\n    value_type: file\n    cardinality: one\n    allowed_mime_groups: [image]\n    allowed_extensions: [png]',
      `record_schema: '{"required": ["manual"]}'\n`,
    ),
  );

  await page.goto(`/records/${record.id}/migrate`);
  await expect(
    page.getByText('Save the record before uploading files.'),
  ).toBeHidden();
  const chooser = page.waitForEvent('filechooser');
  await page
    .getByRole('button', { name: /Add files?|Choose/ })
    .first()
    .click();
  await (
    await chooser
  ).setFiles({
    name: 'manual.png',
    mimeType: 'image/png',
    buffer: png,
  });
  await expect(page.getByText('manual.png')).toBeVisible();
  await page.getByRole('button', { name: 'Upgrade record' }).click();
  await expect(page).toHaveURL(new RegExp(`/records/${record.id}$`));
  await expect(page.getByText('manual.png').first()).toBeVisible();
});

test('a removed member accepts an invitation with their password', async ({
  browser,
}) => {
  const { email, password } = await createMember('viewer');
  const members =
    await request<Array<{ id: string; email: string }>>('/workspace/members');
  const member = members.find((candidate) => candidate.email === email)!;
  await request(`/workspace/members/${member.id}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ state: 'inactive' }),
  });
  const session = await request<{ workspace_id: string }>('/auth/session');
  const roles = await request<Array<{ id: string; code: string }>>(
    '/workspace/assignable-roles',
  );
  await request('/workspace/invitations', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email,
      role_id: roles.find(({ code }) => code === 'viewer')!.id,
      scope_type: 'workspace',
      scope_target_id: session.workspace_id,
      expires_at: new Date(Date.now() + 60 * 60 * 1000).toISOString(),
    }),
  });
  let secret: string | undefined;
  await expect
    .poll(async () => {
      const { messages } = await fetch(`${e2eMailpitUrl}/api/v1/messages`).then(
        (response) => response.json(),
      );
      for (const message of messages) {
        if (
          !message.To.some(
            (recipient: { Address: string }) => recipient.Address === email,
          )
        )
          continue;
        const delivered = await fetch(
          `${e2eMailpitUrl}/api/v1/message/${message.ID}`,
        ).then((response) => response.text());
        secret = delivered.match(/invitations\/accept\?secret=([\w-]+)/)?.[1];
        if (secret) return true;
      }
      return false;
    })
    .toBe(true);

  const context = await browser.newContext({
    storageState: { cookies: [], origins: [] },
  });
  const invited = await context.newPage();
  await invited.goto(`/invitations/accept?secret=${secret}`);
  await invited.getByLabel('Email').fill(email);
  await invited.getByLabel('Password').fill('not their password');
  await invited
    .getByRole('button', { name: 'Sign in with a password to accept' })
    .click();
  await expect(
    invited.getByText('The email or password is incorrect.'),
  ).toBeVisible();
  await invited.getByLabel('Password').fill(password);
  await invited
    .getByRole('button', { name: 'Sign in with a password to accept' })
    .click();
  await expect(invited).toHaveURL(/\/$/);
  await expect(
    invited.getByRole('link', { name: 'Record explorer' }),
  ).toBeVisible();
  await context.close();
});

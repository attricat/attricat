import { expect, test } from '@playwright/test';
import { e2eMailpitUrl } from './ports.ts';
import { suffix } from './helpers.ts';

const futureDateTime = () =>
  new Date(Date.now() + 24 * 60 * 60 * 1000).toISOString().slice(0, 16);

const onboardingUrlFor = async (email: string) => {
  await expect
    .poll(async () => {
      const messages = await fetch(`${e2eMailpitUrl}/api/v1/messages`).then(
        (response) => response.json(),
      );
      return messages.messages.find(
        (candidate: { Subject: string; To: Array<{ Address: string }> }) =>
          candidate.Subject === 'Set up your Catalog workspace account' &&
          candidate.To.some((recipient) => recipient.Address === email),
      )?.ID;
    })
    .toBeTruthy();

  const messages = await fetch(`${e2eMailpitUrl}/api/v1/messages`).then(
    (response) => response.json(),
  );
  const message = messages.messages.find(
    (candidate: { Subject: string; To: Array<{ Address: string }> }) =>
      candidate.Subject === 'Set up your Catalog workspace account' &&
      candidate.To.some((recipient) => recipient.Address === email),
  );
  const delivered = await fetch(
    `${e2eMailpitUrl}/api/v1/message/${message.ID}`,
  ).then((response) => response.json());
  const rawUrl = JSON.stringify(delivered).match(
    /http:\/\/127\.0\.0\.1:\d+\/onboarding\?[^"\\\s]+/,
  )?.[0];
  expect(rawUrl).toBeTruthy();
  return rawUrl!.replaceAll('\\u0026', '&').replaceAll('&amp;', '&');
};

test('manages custom roles through their full lifecycle', async ({ page }) => {
  const roleCode = `catalog_editor_${suffix()}`;
  const renamedCode = `${roleCode}_renamed`;
  const duplicateCode = `${roleCode}_copy`;

  await page.goto('/manage/workspace');
  await expect(page).toHaveURL(/\/manage\/workspace\/?$/);
  await expect(
    page.getByRole('heading', { name: 'Workspace management' }),
  ).toBeVisible();
  await page.getByRole('link', { name: 'Roles' }).click();

  await page.getByLabel('Role code').fill(roleCode);
  await page.getByLabel(/^blueprints\.read —/).check();
  await page.getByRole('button', { name: 'Create role' }).click();

  let role = page.getByRole('listitem').filter({ hasText: roleCode });
  await expect(role).toContainText('blueprints.read');
  await role.getByRole('button', { name: 'Rename role' }).click();
  const renameDialog = page.getByRole('dialog', { name: 'Rename role' });
  await expect(renameDialog.getByLabel('Role code')).toBeFocused();
  await renameDialog.getByLabel('Role code').fill(renamedCode);
  await renameDialog.getByRole('button', { name: 'Rename role' }).click();

  role = page.getByRole('listitem').filter({ hasText: renamedCode });
  await expect(role).toBeVisible();
  await role.getByRole('button', { name: 'Duplicate role' }).click();
  const duplicateDialog = page.getByRole('dialog', { name: 'Duplicate role' });
  await duplicateDialog.getByLabel('New role code').fill(duplicateCode);
  await duplicateDialog.getByRole('button', { name: 'Duplicate role' }).click();

  const duplicate = page
    .getByRole('listitem')
    .filter({ hasText: duplicateCode });
  await expect(duplicate).toContainText('blueprints.read');
  await duplicate.getByRole('button', { name: 'Retire role' }).click();
  const retireDialog = page.getByRole('dialog', { name: 'Retire role' });
  await expect(retireDialog.getByLabel('Replacement role ID')).toBeFocused();
  await retireDialog.getByRole('button', { name: 'Retire role' }).click();
  await expect(duplicate).toBeHidden();
});

test('grants and revokes a workspace member role', async ({ page }) => {
  const roleCode = `fixture_reader_${suffix()}`;

  await page.goto('/manage/workspace/roles');
  await page.getByLabel('Role code').fill(roleCode);
  await page.getByLabel(/^entities\.read —/).check();
  await page.getByRole('button', { name: 'Create role' }).click();
  await expect(
    page.getByRole('listitem').filter({ hasText: roleCode }),
  ).toBeVisible();

  await page.getByRole('link', { name: 'Members' }).click();
  await expect(page.getByText('fixture@example.test').first()).toBeVisible();
  await page.getByLabel('Member').click();
  await page.getByRole('option', { name: 'fixture@example.test' }).click();
  await page.getByLabel('Role').click();
  await page.getByRole('option', { name: roleCode }).click();
  await page.getByRole('button', { name: 'Grant role' }).click();

  const member = page
    .getByRole('listitem')
    .filter({ hasText: 'fixture@example.test' });
  await expect(member).toContainText(roleCode);
  await member.getByRole('button', { name: `Revoke ${roleCode}` }).click();
  await expect(member).not.toContainText(roleCode);
});

test('creates a workspace user and completes onboarding', async ({
  browser,
  page,
}) => {
  const email = `new-user-${suffix()}@example.test`;
  const password = 'new-user-e2e-password';

  await page.goto('/manage/workspace/invitations');
  const form = page
    .getByRole('heading', { name: 'Create user and invite' })
    .locator('xpath=ancestor::form');
  await form.getByLabel('Email').fill(email);
  await form.getByLabel('Display name (optional)').fill('New teammate');
  await form.getByLabel('Role').click();
  await page.getByRole('option', { name: 'viewer' }).click();
  await form.getByLabel('Expires at').fill(futureDateTime());
  await form.getByRole('button', { name: 'Create user and invite' }).click();

  const invitation = page.getByRole('listitem').filter({ hasText: email });
  await expect(invitation).toContainText('viewer');
  const onboardingUrl = await onboardingUrlFor(email);

  const context = await browser.newContext();
  const onboardingPage = await context.newPage();
  await onboardingPage.goto(onboardingUrl);
  await onboardingPage.getByLabel('Password', { exact: true }).fill(password);
  await onboardingPage.getByLabel('Confirm password').fill(password);
  await onboardingPage
    .getByRole('button', { name: 'Set password and join workspace' })
    .click();
  await expect(onboardingPage).toHaveURL(/\/$/);
  await expect(
    onboardingPage.getByRole('link', { name: 'Entity explorer' }),
  ).toBeVisible();
  await expect(
    onboardingPage.getByRole('link', { name: 'Workspace management' }),
  ).toBeHidden();
  await context.close();
});

test('revokes a pending workspace invitation', async ({ page }) => {
  const email = `revoked-user-${suffix()}@example.test`;

  await page.goto('/manage/workspace/invitations');
  const form = page
    .getByRole('heading', { name: 'Create user and invite' })
    .locator('xpath=ancestor::form');
  await form.getByLabel('Email').fill(email);
  await form.getByLabel('Role').click();
  await page.getByRole('option', { name: 'viewer' }).click();
  await form.getByLabel('Expires at').fill(futureDateTime());
  await form.getByRole('button', { name: 'Create user and invite' }).click();

  const invitation = page.getByRole('listitem').filter({ hasText: email });
  await expect(invitation).toBeVisible();
  await invitation.getByRole('button', { name: 'Revoke' }).click();
  await expect(invitation.getByRole('button', { name: 'Revoke' })).toBeHidden();
});

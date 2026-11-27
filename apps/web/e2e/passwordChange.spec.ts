import { expect, type Page, test } from '@playwright/test';
import { createMember } from './helpers.ts';
import { e2eMailpitUrl } from './ports.ts';
import { e2eLoginIdentifier } from './workspace-identity.ts';

// A member of its own, so the shared owner and fixture sessions survive.

const signedOut = { storageState: { cookies: [], origins: [] } };

const signIn = async (page: Page, email: string, password: string) => {
  await page.goto(`/login/${e2eLoginIdentifier}`);
  await page.getByLabel('Email').fill(email);
  await page.getByLabel('Password').fill(password);
  await page.getByRole('button', { name: 'Sign in' }).click();
};

test('a member changes their password from the profile', async ({
  browser,
}) => {
  const member = await createMember('viewer');
  const newPassword = 'a changed e2e password 7!';
  const here = await browser.newContext(signedOut);
  const elsewhere = await browser.newContext(signedOut);
  const page = await here.newPage();
  const otherPage = await elsewhere.newPage();
  await signIn(page, member.email, member.password);
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();
  await signIn(otherPage, member.email, member.password);
  await expect(
    otherPage.getByRole('button', { name: 'Sign out' }),
  ).toBeVisible();

  await page.goto('/profile');
  await page.getByRole('button', { name: 'Change password' }).click();
  const dialog = page.getByRole('dialog', { name: 'Change password' });
  await dialog.getByLabel(/Current password/).fill('not the password 1!');
  await dialog.getByLabel(/^New password/).fill(newPassword);
  await dialog.getByLabel(/Confirm new password/).fill(newPassword);
  await dialog.getByRole('button', { name: 'Change password' }).click();
  await expect(
    dialog.getByText('The current password is incorrect.'),
  ).toBeVisible();

  await dialog.getByLabel(/Current password/).fill(member.password);
  await dialog.getByRole('button', { name: 'Change password' }).click();
  await expect(dialog).toBeHidden();
  await expect(
    page.getByText('Password changed. Other browsers were signed out.'),
  ).toBeVisible();

  // This browser keeps working; the other one is signed out.
  await page.reload();
  await expect(
    page.getByRole('button', { name: 'Change password' }),
  ).toBeVisible();
  await otherPage.reload();
  await expect(otherPage).toHaveURL(/\/login$/);

  const fresh = await (await browser.newContext(signedOut)).newPage();
  await signIn(fresh, member.email, member.password);
  await expect(fresh.getByRole('alert')).toBeVisible();
  await signIn(fresh, member.email, newPassword);
  await expect(fresh.getByRole('button', { name: 'Sign out' })).toBeVisible();

  const notice = async () => {
    const messages = await fetch(`${e2eMailpitUrl}/api/v1/messages`).then(
      (response) => response.json(),
    );
    return messages.messages.some(
      (candidate: { Subject: string; To: Array<{ Address: string }> }) =>
        candidate.Subject === 'Your Attricat password was changed' &&
        candidate.To.some(({ Address }) => Address === member.email),
    );
  };
  await expect.poll(notice).toBe(true);
  await here.close();
  await elsewhere.close();
  await fresh.context().close();
});

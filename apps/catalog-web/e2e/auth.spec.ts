import { expect, test } from '@playwright/test';
import { e2eMailpitUrl } from './ports.ts';

test('signs in and signs out through browser cookies', async ({ page }) => {
  await page.context().clearCookies();
  await page.goto('/');
  await expect(page).toHaveURL(/\/login$/);
  await page.getByLabel('Workspace').fill('default.local');
  await page.getByRole('button', { name: 'Continue' }).click();
  await expect(page).toHaveURL(/\/login\/default\.local$/);

  await page.getByLabel('Email').fill('fixture@example.test');
  await page.getByLabel('Password').fill('e2e-only-fixture-password');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();

  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(page).toHaveURL(/\/login$/);
});

test('resets a password using a Mailpit-delivered one-time link', async ({
  page,
}) => {
  await page.goto('/password-reset');
  await page.getByLabel('Email').fill('owner@example.test');
  await page.getByRole('button', { name: 'Send reset link' }).click();
  await expect(page.getByText(/If an eligible account/)).toBeVisible();

  const messages = await fetch(`${e2eMailpitUrl}/api/v1/messages`).then(
    (response) => response.json(),
  );
  const message = messages.messages.find(
    (candidate: { To: Array<{ Address: string }> }) =>
      candidate.To.some(
        (recipient) => recipient.Address === 'owner@example.test',
      ),
  );
  expect(message).toBeTruthy();
  const delivered = await fetch(
    `${e2eMailpitUrl}/api/v1/message/${message.ID}`,
  ).then((response) => response.json());
  const url = JSON.stringify(delivered).match(
    /http:\/\/127\.0\.0\.1:\d+\/password-reset\/confirm\?token=[A-Za-z0-9_-]+/,
  )?.[0];
  expect(url).toBeTruthy();

  await page.goto(url!);
  await page.getByLabel('New password').fill('new-e2e-owner-password');
  await page.getByRole('button', { name: 'Reset password' }).click();
  await expect(page.getByText(/Your password has been reset/)).toBeVisible();

  await page.goto(url!);
  await page.getByLabel('New password').fill('another-e2e-fixture-password');
  await page.getByRole('button', { name: 'Reset password' }).click();
  await expect(page.getByText(/invalid or expired/)).toBeVisible();

  await page.context().clearCookies();
  await page.goto('/login');
  await page.getByLabel('Workspace').fill('default.local');
  await page.getByRole('button', { name: 'Continue' }).click();
  await page.getByLabel('Email').fill('owner@example.test');
  await page.getByLabel('Password').fill('new-e2e-owner-password');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(/\/$/);
});

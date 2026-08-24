import { expect, test } from '@playwright/test';

test('signs in and signs out through browser cookies', async ({ page }) => {
  await page.context().clearCookies();
  await page.goto('/');
  await expect(page).toHaveURL(/\/login$/);

  await page.getByLabel('Email').fill('fixture@example.test');
  await page.getByLabel('Password').fill('e2e-only-fixture-password');
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(/\/$/);
  await expect(page.getByRole('button', { name: 'Sign out' })).toBeVisible();

  await page.getByRole('button', { name: 'Sign out' }).click();
  await expect(page).toHaveURL(/\/login$/);
});

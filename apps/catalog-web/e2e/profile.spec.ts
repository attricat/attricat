import { expect, test } from '@playwright/test';

test('opens Profile and manages a personal API token', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Profile' }).click();

  await expect(page).toHaveURL(/\/profile$/);
  await expect(page.getByRole('heading', { name: 'Profile' })).toBeVisible();
  await expect(page.getByText('Email: owner@example.test')).toBeVisible();

  await page.getByLabel('Label').fill('playwright token');
  await page.getByLabel('blueprints.read').check();
  await page.getByRole('button', { name: 'Create token' }).click();
  await expect(page.getByText('Copy this secret now')).toBeVisible();
  await expect(page.locator('input[value^="cat_pat_"]')).toBeVisible();
  await page.keyboard.press('Escape');

  const token = page
    .getByRole('listitem')
    .filter({ hasText: 'playwright token' });
  await expect(token).toContainText('Last used: Never');
  await token.getByRole('button', { name: 'Revoke' }).click();
  await expect(token).toContainText('Revoked:');
});

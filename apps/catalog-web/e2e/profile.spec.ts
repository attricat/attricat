import { expect, test } from '@playwright/test';

test('opens Profile and manages a personal API token', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('link', { name: 'Profile' }).click();

  await expect(page).toHaveURL(/\/profile$/);
  await expect(page.getByRole('heading', { name: 'Profile' })).toBeVisible();
  await expect(page.getByText('owner@example.test')).toBeVisible();

  await page.getByRole('tab', { name: 'Personal API tokens' }).click();
  await expect(page).toHaveURL(/\/profile\/personal-access-tokens$/);
  await page.getByRole('link', { name: 'Create personal token' }).click();
  await expect(page).toHaveURL(/\/profile\/personal-access-tokens\/new$/);
  await page.getByLabel('Label').fill('playwright token');
  await page.getByLabel('blueprints.read').check();
  await page.getByRole('button', { name: 'Create token' }).click();
  await expect(page.getByText('Copy this secret now')).toBeVisible();
  await expect(page.locator('input[value^="cat_pat_"]')).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(page).toHaveURL(/\/profile\/personal-access-tokens$/);

  const token = page
    .getByRole('listitem')
    .filter({ hasText: 'playwright token' });
  await expect(token.locator('dt:text-is("Last used") + dd')).toHaveText(
    'Never',
  );
  await token.getByRole('button', { name: 'Revoke' }).click();
  const confirm = page.getByRole('dialog', { name: 'Revoke personal token?' });
  await expect(confirm).toContainText('playwright token');
  await confirm.getByRole('button', { name: 'Revoke token' }).click();
  await expect(confirm).toBeHidden();
  await expect(token.locator('dt:text-is("Revoked") + dd')).toBeVisible();
});

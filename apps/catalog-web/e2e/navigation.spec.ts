import { expect, test } from '@playwright/test';

test('opens mobile navigation and closes it after navigation', async ({
  page,
}) => {
  await page.goto('/');

  await page.getByRole('button', { name: 'Open navigation' }).click();
  await page.getByRole('button', { name: 'Manage' }).click();
  const blueprintsLink = page.getByRole('link', { name: 'Blueprints' });
  await expect(blueprintsLink).toBeVisible();
  await blueprintsLink.click();

  await expect(page).toHaveURL(/\/manage\/blueprints$/);
  await expect(blueprintsLink).toBeHidden();
  await expect(page.getByRole('heading', { name: 'Blueprints' })).toBeVisible();
});

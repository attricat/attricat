import { expect, test } from '@playwright/test';

test('navigates from the desktop rail and management panel', async ({
  page,
}) => {
  await page.setViewportSize({ width: 1280, height: 800 });
  await page.goto('/');
  await expect(
    page.getByRole('navigation', { name: /entity explorer/i }),
  ).toBeVisible();
  const blueprintSelector = page.getByRole('combobox', {
    name: /select a blueprint/i,
  });
  await expect(blueprintSelector).toHaveCount(1);
  await expect(blueprintSelector).toBeVisible();

  await expect(
    page.getByRole('button', { name: 'Open navigation' }),
  ).toBeHidden();
  await page.getByRole('link', { name: 'Manage' }).click();
  const blueprintsLink = page.getByRole('link', {
    exact: true,
    name: 'Blueprints',
  });
  await expect(blueprintsLink).toBeVisible();
  await blueprintsLink.click();

  await expect(page).toHaveURL(/\/manage\/blueprints$/);
  await expect(page.getByRole('heading', { name: 'Blueprints' })).toBeVisible();
});

test('opens mobile navigation and closes it after navigation', async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto('/');
  await page.getByRole('button', { name: 'Open navigation' }).click();
  await expect(page.locator('.MuiDrawer-paper')).toHaveCSS('width', '264px');
  const backButton = page.getByRole('button', {
    name: 'Back to main navigation',
  });
  const allEntitiesLink = page.getByRole('link', { name: 'All entities' });
  await expect(backButton).toBeVisible();
  await expect(allEntitiesLink).toBeVisible();
  const allEntitiesLinkBox = await allEntitiesLink.boundingBox();
  expect(allEntitiesLinkBox?.width).toBeGreaterThan(220);
  expect(allEntitiesLinkBox?.height).toBeLessThan(80);
  const blueprintSelector = page.getByRole('combobox', {
    name: /select a blueprint/i,
  });
  await expect(blueprintSelector).toHaveCount(1);
  await expect(blueprintSelector).toBeVisible();
  await expect(page.getByRole('link', { name: 'Agents' })).toBeHidden();
  await backButton.click();

  await page.getByRole('button', { name: 'Manage' }).click();
  const blueprintsLink = page.getByRole('link', {
    exact: true,
    name: 'Blueprints',
  });
  await expect(backButton).toBeVisible();
  await expect(page.getByRole('link', { name: 'Agents' })).toBeHidden();
  await expect(blueprintsLink).toBeVisible();

  await backButton.click();
  await expect(backButton).toBeHidden();
  await expect(blueprintsLink).toBeHidden();
  await expect(page.getByRole('link', { name: 'Agents' })).toBeVisible();

  await page.getByRole('button', { name: 'Manage' }).click();
  await expect(blueprintsLink).toBeVisible();
  await blueprintsLink.click();

  await expect(page).toHaveURL(/\/manage\/blueprints$/);
  await expect(blueprintsLink).toBeHidden();
  await expect(page.getByRole('heading', { name: 'Blueprints' })).toBeVisible();
});

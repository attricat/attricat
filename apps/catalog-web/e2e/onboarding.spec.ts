import { expect, test } from '@playwright/test';
import { createEntityBlueprint, suffix } from './helpers.ts';

test('guides an administrator from onboarding to the explorer', async ({
  page,
}) => {
  // The shared E2E workspace accumulates blueprints from other specs, so
  // simulate a fresh workspace for the onboarding half of the journey.
  await page.route('**/api/blueprints/catalogue', (route) =>
    route.fulfill({ json: [] }),
  );
  await page.goto('/');

  await expect(
    page.getByRole('heading', { name: 'Welcome to Attricat' }),
  ).toBeVisible();
  await expect(
    page.getByRole('link', { name: 'Read the documentation' }),
  ).toHaveAttribute('href', 'https://docs.attricat.com/');
  await expect(
    page.getByRole('link', {
      name: 'Read the guide: Create your first blueprint',
    }),
  ).toHaveAttribute('href', 'https://docs.attricat.com/builders/blueprints/');

  await page.getByRole('link', { name: 'Create blueprint' }).click();
  await expect(page).toHaveURL(/\/manage\/blueprints\/new$/);

  await page.unroute('**/api/blueprints/catalogue');
  const id = suffix();
  await createEntityBlueprint(
    `onboarding_${id}`,
    `Onboarding ${id}`,
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
  );
  await page.goto('/');

  await expect(
    page.getByRole('combobox', { name: /select a blueprint/i }),
  ).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Welcome to Attricat' }),
  ).toHaveCount(0);
});

import { expect, test } from '@playwright/test';
import {
  createContext,
  createEntity,
  createEntityBlueprint,
  defaultContext,
  scalar,
  suffix,
} from './helpers';

test('shows health sections, custom stale threshold, and refreshes data', async ({
  page,
}) => {
  const code = `health_${suffix()}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Healthy product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"',
    { entitySchema: '{"type":"object","required":["title"]}' },
  );
  await createEntity(blueprint, [scalar('title', 'Health check')]);
  const context = await createContext(
    `health_context_${suffix()}`,
    (await defaultContext()).id,
  );

  await page.goto('/manage/data-health');
  await expect(
    page.getByRole('heading', { name: 'Data health' }),
  ).toBeVisible();
  await expect(page.getByText(`Healthy product (${code}) v1`)).toBeVisible();
  await expect(
    page.getByRole('heading', { name: 'Freshness distribution' }),
  ).toBeVisible();
  await expect(page.getByRole('heading', { name: 'Storage' })).toBeVisible();

  await page.getByLabel('Stale after').click();
  await page.getByRole('option', { name: 'Custom' }).click();
  await page.getByLabel('Days').fill('45');
  await page.getByLabel('Days').blur();
  await expect(page).toHaveURL(/staleAfterDays=45/);
  await expect(page.getByText('Stale after 45 days')).toBeVisible();

  await page.getByRole('button', { name: 'Default completeness' }).click();
  await expect(page.getByText(`Healthy product (${code}) v1`)).toBeVisible();
  await page.getByRole('button', { name: 'Context coverage' }).click();
  await expect(page.getByText(context.code)).toBeVisible();
  await page.getByRole('button', { name: 'Relationship integrity' }).click();
  await expect(page.getByRole('button', { name: 'Refresh' })).toBeEnabled();
  await page.getByRole('button', { name: 'Refresh' }).click();
});

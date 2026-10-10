import { expect, test } from '@playwright/test';
import { request, signInAsMember, suffix } from './helpers';

test('shows background processing and the running build', async ({ page }) => {
  await page.goto('/manage/background-processing');
  await expect(
    page.getByRole('heading', { name: 'Background processing' }),
  ).toBeVisible();
  const updated = page.getByText('Last updated at');
  await expect(updated).toBeVisible();
  await expect(
    page.getByText('Could not load background processing status.', {
      exact: false,
    }),
  ).toBeHidden();
  await page.getByRole('button', { name: 'Refresh' }).click();
  await expect(updated).toBeVisible();

  await page.goto('/manage/system-health');
  const build = page.getByRole('region', { name: 'Running build' });
  await expect(build.getByRole('term')).toContainText([
    'API version',
    'Branch',
    'Commit',
  ]);
  await expect(build.getByRole('definition').last()).toHaveText(/\S/);
});

test('refuses operations pages to a role without data-health access', async ({
  browser,
}) => {
  const roleCode = `reader_${suffix()}`;
  await request('/workspace/roles', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ code: roleCode, permissions: ['records.read'] }),
  });
  const page = await signInAsMember(browser, roleCode);

  await page.goto('/manage/background-processing');
  await expect(page.getByRole('alert')).toHaveText(
    'You are not authorized to view background processing in this workspace.',
  );
  const status = await page.request.get(
    '/api/data-health/background-processing',
  );
  expect(status.status()).toBe(403);

  await page.goto('/manage/system-health');
  await expect(page.getByRole('alert')).toHaveText(
    'You are not authorized to view system health in this workspace.',
  );
  await page.context().close();
});

import { expect, test } from '@playwright/test';

// A fixed browser zone makes the preferred zone observably different.
test.use({ locale: 'en-US', timezoneId: 'UTC' });

test('renders timestamps in the preferred time zone', async ({ page }) => {
  await page.goto('/profile');
  const picker = page.getByRole('combobox', { name: 'Time zone' });
  await expect(picker).toHaveValue('Automatic (browser: UTC)');
  const preview = page.getByText(/^Current time:/);
  await expect(preview).not.toContainText('GMT+9');

  await picker.fill('Asia/Tokyo');
  await page.getByRole('option', { name: /^Asia\/Tokyo / }).click();
  await expect(picker).toHaveValue(/^Asia\/Tokyo /);
  await expect(preview).toContainText('GMT+9');

  const time = preview.locator('time');
  await time.hover();
  await expect(page.getByRole('tooltip')).toHaveText(
    /^\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2} UTC$/,
  );

  // The preference belongs to the account, not the page.
  await page.reload();
  await expect(picker).toHaveValue(/^Asia\/Tokyo /);
  await expect(preview).toContainText('GMT+9');

  await picker.fill('Automatic');
  await page.getByRole('option', { name: /^Automatic/ }).click();
  await expect(picker).toHaveValue('Automatic (browser: UTC)');
  await expect(preview).not.toContainText('GMT+9');
});

import { expect, test } from '@playwright/test';
import {
  createEntity,
  createEntityBlueprint,
  replaceDefinition,
  scalar,
  suffix,
} from './helpers';

test('authors, enables, runs and disables a manual workflow', async ({
  page,
}) => {
  const code = `workflow_${suffix()}`;
  const workflowCode = `review-${suffix()}`;
  const workflowName = `Mark reviewed ${workflowCode}`;
  const blueprint = await createEntityBlueprint(
    code,
    'Workflow product',
    '[[attributes]]\ncode = "title"\nvalue_type = "string"\n\n[[attributes]]\ncode = "review_state"\nvalue_type = "string"',
  );
  const entity = await createEntity(blueprint, [
    scalar('title', 'Workflow subject'),
    scalar('review_state', 'pending'),
  ]);

  await page.goto('/manage/workflows');
  await page.getByRole('link', { name: 'New workflow' }).click();
  await replaceDefinition(
    page,
    `format_version = 2
code = "${workflowCode}"
name = "${workflowName}"

[[triggers]]
type = "manual"

[[actions]]
type = "attribute_write"
attribute_code = "review_state"
fixed = "reviewed"
`,
  );
  await page.getByRole('button', { name: 'Validate TOML' }).click();
  await expect(
    page.getByText(`Valid: ${workflowName} has 1 triggers and 1 actions.`),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Save draft' }).click();

  await expect(page).toHaveURL(/\/manage\/workflows\/[0-9a-f-]{36}$/);
  const workflowUrl = page.url();
  await expect(page.getByRole('heading', { name: workflowName })).toBeVisible();
  await expect(page.getByText('Draft', { exact: true }).first()).toBeVisible();
  await page.getByRole('button', { name: 'Publish', exact: true }).click();
  await expect(
    page.getByText('Published', { exact: true }).first(),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Enable', exact: true }).click();
  await expect(page.getByText('Enabled revision v1')).toBeVisible();

  await page.getByLabel('Manual run entity ID').fill(entity.id);
  await page.getByRole('button', { name: 'Run now' }).click();
  await expect(async () => {
    await page.reload();
    await page.getByRole('tab', { name: 'Run diagnostics' }).click();
    await expect(
      page.getByRole('row').filter({ hasText: 'Completed' }),
    ).toBeVisible({ timeout: 1_000 });
  }).toPass();

  await page.goto(`/entities/${entity.id}`);
  await expect(page.getByRole('textbox', { name: 'review state' })).toHaveValue(
    'reviewed',
  );

  await page.goto('/manage/workflows');
  await page.getByRole('link', { name: workflowName }).click();
  await expect(page).toHaveURL(workflowUrl);
  await page.getByRole('button', { name: 'Disable', exact: true }).click();
  await expect(page.getByText('Disabled', { exact: true })).toBeVisible();
  await expect(page.getByLabel('Manual run entity ID')).toBeHidden();
});

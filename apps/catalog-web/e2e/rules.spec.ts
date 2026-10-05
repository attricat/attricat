import { expect, test, type Page } from '@playwright/test';
import { createEntity, createEntityBlueprint, scalar, suffix } from './helpers';

// Run history is listed newest first.
const expectLatestRun = async (page: Page, ruleName: string, run: string) => {
  await expect(async () => {
    await page.goto('/manage/rules/runs');
    await expect(
      page.getByRole('row').filter({ hasText: ruleName }).first(),
    ).toHaveAccessibleName(`${ruleName} v1 All entities ${run}`, {
      timeout: 1_000,
    });
  }).toPass();
};

const openFindings = async (page: Page) => {
  const loaded = page.waitForResponse(
    (response) =>
      new URL(response.url()).pathname === '/api/rule-findings' &&
      response.ok(),
  );
  await page.goto('/manage/rules/findings');
  await loaded;
};

test('dry-runs and runs a rule, then acknowledges and resolves its finding', async ({
  page,
}) => {
  const code = `ruled_${suffix()}`;
  const attribute = `summary_${suffix()}`;
  const ruleName = `Summary required ${code}`;
  const finding = `'${attribute}' is required`;
  const blueprint = await createEntityBlueprint(
    code,
    'Ruled product',
    `[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "${attribute}"
value_type = "string"

[[rules]]
code = "summary-required-${suffix()}"
name = "${ruleName}"
severity = "error"

[[rules.triggers]]
type = "manual"

[rules.predicate]
type = "required"
attribute_code = "${attribute}"`,
  );
  await createEntity(blueprint, [
    scalar('title', 'Complete product'),
    scalar(attribute, 'Has a summary'),
  ]);
  const missing = await createEntity(blueprint, [
    scalar('title', 'Incomplete product'),
  ]);
  // Blueprint rules publish disabled, so a normal run is refused.
  await page.goto('/manage/rules');
  const ruleRow = page.getByRole('row').filter({ hasText: ruleName });
  await expect(ruleRow).toContainText('Disabled');
  await ruleRow.getByRole('button', { name: 'Run now' }).click();
  await expect(page.getByRole('alert')).toContainText(
    'rule has no enabled revision to run',
  );

  const toggle = ruleRow.getByRole('switch', {
    name: `Enable ${ruleName} v1`,
  });
  // The switch reflects the saved lifecycle, so it flips after the response.
  await toggle.click();
  await expect(toggle).toBeChecked();
  await expect(ruleRow).toContainText('Enabled');
  await expect(
    page.getByText('rule has no enabled revision to run'),
  ).toBeHidden();
  await page.reload();
  await expect(toggle).toBeChecked();
  await ruleRow.getByRole('button', { name: 'Dry run' }).click();
  await expectLatestRun(page, ruleName, 'manual (dry run) Completed 2 1');
  await openFindings(page);
  await expect(page.getByRole('cell', { name: finding })).toBeHidden();

  await page.goto('/manage/rules');
  await ruleRow.getByRole('button', { name: 'Run now' }).click();
  await expectLatestRun(page, ruleName, 'manual Completed 2 1');

  await openFindings(page);
  const findingRow = page.getByRole('row').filter({ hasText: finding });
  await expect(findingRow).toContainText('Error');
  await expect(findingRow).toContainText('Open');
  await expect(findingRow).toContainText(`${ruleName} v1`);
  await expect(
    findingRow.getByRole('link', { name: `Open entity ${missing.id}` }),
  ).toBeVisible();
  await findingRow.getByRole('button', { name: 'Acknowledge' }).click();
  await expect(findingRow).toContainText('Acknowledged');
  await expect(
    findingRow.getByRole('button', { name: 'Acknowledge' }),
  ).toBeHidden();

  await page.goto(`/entities/${missing.id}`);
  await expect(page.getByText('1 data quality finding')).toBeVisible();
  await page.goto(`/entities/${missing.id}/edit`);
  // Form labels show attribute codes with spaces in place of underscores.
  await page.getByLabel(attribute.replace('_', ' ')).fill('Fixed summary');
  await page.getByRole('button', { name: 'Save changes' }).click();
  await expect(page).toHaveURL(new RegExp(`/entities/${missing.id}$`));

  await page.goto('/manage/rules');
  await ruleRow.getByRole('button', { name: 'Run now' }).click();
  await expectLatestRun(page, ruleName, 'manual Completed 2 0');
  await openFindings(page);
  await expect(findingRow).toBeHidden();
  await page.goto(`/entities/${missing.id}`);
  await expect(page.getByText('Fixed summary')).toBeVisible();
  await expect(page.getByText('1 data quality finding')).toBeHidden();

  await page.goto('/manage/rules');
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await expect(ruleRow).toContainText('Disabled');
  await page.reload();
  await expect(toggle).not.toBeChecked();
});

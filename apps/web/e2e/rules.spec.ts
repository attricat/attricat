import { expect, test, type Page } from '@playwright/test';
import {
  commitField,
  createRecord,
  createRecordBlueprint,
  createRule,
  enableRule,
  scalar,
  suffix,
} from './helpers';

// Run history is listed newest first. Saving a record also runs the enabled
// rules for that record alone, and that run may be listed after a manual one,
// so only runs over all records count.
const expectLatestRun = async (page: Page, ruleName: string, run: string) => {
  await expect(async () => {
    await page.goto('/manage/rules/runs');
    await expect(
      page
        .getByRole('row')
        .filter({ hasText: ruleName })
        .filter({ hasText: 'All records' })
        .first(),
    ).toHaveAccessibleName(`${ruleName} v1 All records ${run}`, {
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
  const blueprint = await createRecordBlueprint(
    code,
    'Ruled product',
    `[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "${attribute}"
value_type = "string"`,
  );
  await createRule(
    blueprint,
    `code = "summary-required-${suffix()}"
name = "${ruleName}"
severity = "error"

[[triggers]]
type = "manual"

[predicate]
type = "required"
attribute_code = "${attribute}"`,
  );
  await createRecord(blueprint, [
    scalar('title', 'Complete product'),
    scalar(attribute, 'Has a summary'),
  ]);
  const missing = await createRecord(blueprint, [
    scalar('title', 'Incomplete product'),
  ]);
  // New rules start disabled, so a normal run is refused.
  await page.goto('/manage/rules');
  const ruleRow = page.getByRole('row').filter({ hasText: ruleName });
  await expect(ruleRow).toContainText('Disabled');
  await ruleRow.getByRole('button', { name: 'Run now' }).click();
  await expect(page.getByRole('alert')).toContainText(
    'The rule is not enabled.',
  );

  const toggle = ruleRow.getByRole('switch', {
    name: `Enable ${ruleName}`,
  });
  // The switch reflects the saved lifecycle, so it flips after the response.
  await toggle.click();
  await expect(toggle).toBeChecked();
  await expect(ruleRow).toContainText('Enabled');
  await expect(page.getByText('The rule is not enabled.')).toBeHidden();
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
    findingRow.getByRole('link', { name: 'Incomplete product' }),
  ).toHaveAttribute('href', `/records/${missing.id}`);
  await findingRow.getByRole('button', { name: 'Acknowledge' }).click();
  await expect(findingRow).toContainText('Acknowledged');
  await expect(
    findingRow.getByRole('button', { name: 'Acknowledge' }),
  ).toBeHidden();

  await page.goto(`/records/${missing.id}`);
  await expect(page.getByText('1 data quality finding')).toBeVisible();
  // Field labels show attribute codes with spaces in place of underscores.
  const summary = page.getByLabel(attribute.replace('_', ' '));
  await summary.fill('Fixed summary');
  await commitField(page, missing.id, summary);

  await page.goto('/manage/rules');
  await ruleRow.getByRole('button', { name: 'Run now' }).click();
  await expectLatestRun(page, ruleName, 'manual Completed 2 0');
  await openFindings(page);
  await expect(findingRow).toBeHidden();
  await page.goto(`/records/${missing.id}`);
  await expect(page.getByLabel(attribute.replace('_', ' '))).toHaveValue(
    'Fixed summary',
  );
  await expect(page.getByText('1 data quality finding')).toBeHidden();

  await page.goto('/manage/rules');
  await toggle.click();
  await expect(toggle).not.toBeChecked();
  await expect(ruleRow).toContainText('Disabled');
  await page.reload();
  await expect(toggle).not.toBeChecked();
});

test('filters records by data quality', async ({ page }) => {
  const code = `quality_${suffix()}`;
  const ruleCode = `summary-required-${suffix()}`;
  const blueprint = await createRecordBlueprint(
    code,
    'Quality product',
    `[[attributes]]
code = "title"
value_type = "string"

[[attributes]]
code = "summary"
value_type = "string"`,
  );
  await enableRule(
    await createRule(
      blueprint,
      `code = "${ruleCode}"
name = "Summary required ${code}"
severity = "error"

[[triggers]]
type = "manual"

[predicate]
type = "required"
attribute_code = "summary"`,
    ),
  );
  // Saving a record checks it against every enabled rule of its revision.
  await createRecord(blueprint, [
    scalar('title', 'Complete product'),
    scalar('summary', 'Has a summary'),
  ]);
  await createRecord(blueprint, [scalar('title', 'Incomplete product')]);

  const rows = page.getByRole('row');
  const incomplete = rows.filter({ hasText: 'Incomplete product' });
  const complete = rows.filter({ hasText: 'Complete product' }).filter({
    hasNotText: 'Incomplete',
  });
  await page.goto(`/?blueprint=${code}`);
  const filters = page.getByRole('group', { name: 'Filters' });
  await filters.getByRole('button', { name: 'Data quality' }).click();
  await page.getByRole('menuitem', { name: 'Open findings' }).click();
  await expect(filters.getByText('Data quality: Open findings')).toBeVisible();
  // Findings arrive once the record's checks finish.
  await expect(async () => {
    await page.reload();
    await expect(incomplete).toBeVisible({ timeout: 1_000 });
  }).toPass();
  await expect(complete).toBeHidden();
});

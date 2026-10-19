import { expect, test, type Page } from '@playwright/test';
import { replaceDefinition, suffix } from './helpers';

const openInsertGroup = async (page: Page, group: string) => {
  await page.getByRole('button', { name: 'Insert' }).click();
  await page.getByRole('menuitem', { name: new RegExp(`^${group}`) }).click();
};

const insert = async (page: Page, group: string, label: string) => {
  await openInsertGroup(page, group);
  await page.getByRole('menuitem', { name: new RegExp(`^${label}`) }).click();
  // Leave the snippet's placeholders at their defaults.
  await page.keyboard.press('Escape');
};

// The emulated Desktop Chrome device reports Windows, so Monaco binds Ctrl+S
// even when the tests run on macOS.
const saveShortcut = (page: Page) => page.keyboard.press('Control+s');

const preview = (page: Page) => page.getByRole('region', { name: 'Preview' });

test('checks, previews and inserts into a blueprint definition', async ({
  page,
}) => {
  await page.goto('/manage/blueprints/new');
  await page.getByRole('button', { name: 'Dismiss', exact: true }).click();
  await expect(page.getByText('Ready to save')).toBeVisible();
  await expect(preview(page).getByRole('tab', { name: 'table' })).toBeVisible();

  const name = preview(page).getByLabel('Name');
  await name.fill('x'.repeat(201));
  await expect(name).toHaveAttribute('aria-invalid', 'true');

  await openInsertGroup(page, 'Views');
  await expect(
    page.getByRole('menuitem', { name: /Table view/ }),
  ).toBeDisabled();
  await page.keyboard.press('Escape');

  await insert(page, 'Views', 'Explorer view');
  await expect(page.getByText(/Problem on line \d+/)).toBeVisible();
  await expect(preview(page).getByRole('alert')).toContainText('browse_by');
  await expect(
    preview(page).getByText(
      'Showing the last version that passed the save checks.',
    ),
  ).toBeVisible();

  const code = `checked_${suffix()}`;
  await replaceDefinition(
    page,
    `format_version = 1\ncode = "${code}"\nname = "Checked"\nkind = "record"\n\n[views.dropdown_option]\ntype = "dropdown_option"\nfields = ["title"]\n\n[[attributes]]\ncode = "title"\nvalue_type = "string"\n`,
  );
  await expect(page.getByText('Ready to save')).toBeVisible();
  await expect(preview(page).getByLabel('title')).toBeVisible();
  await saveShortcut(page);
  await expect(page).toHaveURL(/\/manage\/blueprints\/[0-9a-f-]{36}$/);
});

test('checks, previews and inserts into a reusable attribute definition', async ({
  page,
}) => {
  await page.goto('/manage/reusable-attributes/new');
  await page.getByRole('button', { name: /Choice/ }).click();
  await expect(page.getByText('Ready to save')).toBeVisible();
  await expect(preview(page).getByText(/^string · .+:size$/)).toBeVisible();
  await expect(preview(page).getByText('Explorer filter')).toBeVisible();
  // A short list of choices is radio buttons.
  await preview(page).getByRole('radio', { name: 'Medium' }).check();
  await expect(
    preview(page).getByRole('radio', { name: 'Medium' }),
  ).toBeChecked();

  await openInsertGroup(page, 'Values');
  await expect(
    page.getByRole('menuitem', { name: /Allowed values/ }),
  ).toBeDisabled();
  await page.keyboard.press('Escape');

  await insert(page, 'Behavior', 'Include in search');
  await expect(page.getByText('Ready to save')).toBeVisible();
  await expect(preview(page).getByText('Searchable')).toBeVisible();

  const code = `checked_${suffix()}`;
  await replaceDefinition(
    page,
    `code = "${code}"\nname = "Checked"\nvalue_type = "colour"\n`,
  );
  await expect(page.getByText('Problem on line 3')).toBeVisible();

  await replaceDefinition(
    page,
    `code = "${code}"\nname = "Checked"\nvalue_type = "number"\n`,
  );
  await expect(page.getByText('Ready to save')).toBeVisible();
  await expect(
    preview(page).getByText(`number · `, { exact: false }),
  ).toBeVisible();
  await saveShortcut(page);
  await expect(page).toHaveURL(/\/manage\/reusable-attributes$/);
  await expect(page.getByText(code).first()).toBeVisible();
});

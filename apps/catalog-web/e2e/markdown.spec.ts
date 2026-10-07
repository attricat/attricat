import { expect, test } from '@playwright/test';
import {
  commitField,
  createEntity,
  createEntityBlueprint,
  scalar,
  suffix,
} from './helpers';

for (const mode of ['light', 'dark'] as const) {
  test(`shows a Markdown field formatted until it is edited in ${mode} mode`, async ({
    page,
  }, testInfo) => {
    await page.addInitScript(
      (value) => localStorage.setItem('attricat.color-mode', value),
      mode,
    );
    const code = `markdown_${suffix()}`;
    const blueprint = await createEntityBlueprint(
      code,
      'Markdown samples',
      `
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "notes"
value_type = "string"
`,
      {
        views: `
[views.detail]
type = "stack"
children = [{ type = "field", field = "title" }, { type = "field", field = "notes", component = { id = "catalog.markdown_display", version = 1 } }]
`,
      },
    );
    const entity = await createEntity(blueprint, [
      scalar('title', 'Lamp'),
      scalar('notes', '## Care\n\nWipe with a **dry** cloth.'),
    ]);
    await page.goto(`/entities/${entity.id}`);

    const notes = page.getByRole('textbox', { name: 'notes', exact: true });
    const edit = page.getByRole('button', { name: 'Edit notes' });
    await expect(page.getByRole('heading', { name: 'Care' })).toBeVisible();
    await expect(page.locator('strong', { hasText: 'dry' })).toBeVisible();
    await expect(notes).toHaveCount(0);
    await page.screenshot({
      path: testInfo.outputPath(`markdown-at-rest-${mode}.png`),
    });

    await edit.click();
    await expect(notes).toBeFocused();
    await expect(notes).toHaveValue('## Care\n\nWipe with a **dry** cloth.');
    await notes.pressSequentially(' Avoid *water*.');
    await page.screenshot({
      path: testInfo.outputPath(`markdown-editing-${mode}.png`),
    });
    await commitField(page, entity.id, notes);
    await expect(
      page.getByText('Wipe with a dry cloth. Avoid water.', { exact: true }),
    ).toBeVisible();
    await expect(page.locator('em', { hasText: 'water' })).toBeVisible();
    await expect(notes).toHaveCount(0);

    // Escape drops the edit and returns to the formatted value.
    await edit.click();
    await notes.pressSequentially(' Draft');
    await notes.press('Escape');
    await expect(notes).toHaveCount(0);
    await expect(edit).toBeFocused();
    await page.reload();
    await expect(page.locator('em', { hasText: 'water' })).toBeVisible();
    await expect(page.getByText('Draft')).toHaveCount(0);
  });
}

import { expect, test } from '@playwright/test';
import { createEntity, createEntityBlueprint, scalar, suffix } from './helpers';

for (const mode of ['light', 'dark'] as const) {
  test(`creates, renders and edits Markdown comments in ${mode} mode`, async ({
    page,
  }, testInfo) => {
    const blueprint = await createEntityBlueprint(
      `comments_${suffix()}`,
      'Commented entities',
      '[[attributes]]\ncode = "title"\nvalue_type = "string"',
    );
    const entity = await createEntity(blueprint, [
      scalar('title', 'Comments example'),
    ]);
    await page.addInitScript(
      (mode) => localStorage.setItem('attricat.color-mode', mode),
      mode,
    );
    if (mode === 'dark')
      await page.setViewportSize({ width: 390, height: 844 });
    await page.goto(`/entities/${entity.id}`);
    const panel = page.getByRole('region', { name: 'Comments', exact: true });
    const input = panel.getByRole('textbox', { name: /^Comment/ });
    await expect(input).toBeEnabled();
    await input.fill(
      '**Hello** from a comment\n\n[Safe link](https://example.com)\n\n<script>alert(1)</script>',
    );
    await panel.getByRole('button', { name: 'Preview Markdown' }).click();
    await expect(panel.locator('strong')).toHaveText('Hello');
    await panel.getByRole('button', { name: 'Post comment' }).click();
    await expect(panel.getByRole('status')).toHaveText('Comment saved.');
    await expect(panel.locator('li strong')).toHaveText('Hello');
    await expect(panel.locator('script')).toHaveCount(0);
    await page.reload();
    await expect(panel.locator('li strong')).toHaveText('Hello');
    await panel.getByRole('button', { name: 'Edit', exact: true }).click();
    await panel
      .getByRole('textbox', { name: 'Edit comment' })
      .fill('Updated **Markdown**');
    await panel.getByRole('button', { name: 'Save changes' }).click();
    await expect(panel.locator('li strong')).toHaveText('Markdown');
    await expect(panel.getByText('Edited', { exact: false })).toBeVisible();
    await expect(
      panel.getByRole('button', { name: 'Edit', exact: true }),
    ).toBeFocused();
    await page.screenshot({
      path: testInfo.outputPath(`comments-${mode}.png`),
      fullPage: true,
    });
  });
}

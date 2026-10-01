import { expect, test } from '@playwright/test';
import { createEntity, createEntityBlueprint, scalar, suffix } from './helpers';

// Small valid PNG; the real upload and image worker generate both variants.
const png = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAEAAAAAoCAIAAADBrGu+AAAAPUlEQVR42u3PQQkAAAgEsItuBCMY1Qw+hcEKLNXzWgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELhZHmeDxWtWjYgAAAABJRU5ErkJggg==',
  'base64',
);

for (const mode of ['light', 'dark'] as const) {
  test(`image gallery uploads, zooms and edits attachments in ${mode} mode`, async ({
    page,
  }, testInfo) => {
    await page.emulateMedia({ colorScheme: mode });
    if (mode === 'dark')
      await page.setViewportSize({ width: 390, height: 844 });
    const blueprint = await createEntityBlueprint(
      `photos_${suffix()}`,
      'Gallery example',
      `
[[attributes]]
code = "title"
value_type = "string"
[[attributes]]
code = "photos"
value_type = "file"
cardinality = "many"
ordered = true
image_only = true
allowed_mime_groups = ["image"]
max_bytes = 1048576
`,
    );
    const entity = await createEntity(blueprint, [
      scalar('title', 'Gallery test'),
    ]);
    await page.goto(`/entities/${entity.id}/edit`);
    await page.getByLabel('title', { exact: true }).fill('Unsaved title');
    await page.locator('input[type="file"]').setInputFiles([
      { name: 'front.png', mimeType: 'image/png', buffer: png },
      { name: 'back.png', mimeType: 'image/png', buffer: png },
    ]);
    await page.getByRole('button', { name: 'Upload 2 files' }).click();
    await expect(
      page.getByRole('button', { name: 'Preview front.png' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Preview back.png' }),
    ).toBeVisible();
    await expect(page.getByLabel('title', { exact: true })).toHaveValue(
      'Unsaved title',
    );
    await page.getByRole('button', { name: 'Move front.png later' }).click();
    await expect(
      page.getByRole('button', { name: 'Move front.png earlier' }),
    ).toBeEnabled();
    await page.reload();
    await page.getByRole('button', { name: 'Discard draft' }).click();
    const previews = page.getByRole('button', { name: /^Preview .*\.png$/ });
    await expect(previews.first()).toHaveAccessibleName('Preview back.png');
    // Read-only display retains viewing/zoom without exposing editing controls.
    await page.goto(`/entities/${entity.id}`);
    await expect(
      page.getByRole('button', { name: 'Remove front.png' }),
    ).toHaveCount(0);
    const trigger = page.getByRole('button', { name: 'Preview back.png' });
    await trigger.focus();
    await page.keyboard.press('Enter');
    const dialog = page.getByRole('dialog');
    await expect(
      dialog.getByRole('img', { name: 'back.png', exact: true }),
    ).toBeVisible({ timeout: 30_000 });
    await expect(
      dialog.getByRole('img', { name: 'back.png', exact: true }),
    ).toHaveAttribute('src', /\/variants\/display\/download$/);
    await page.screenshot({ path: testInfo.outputPath(`gallery-${mode}.png`) });
    await dialog.getByRole('button', { name: 'Zoom in', exact: true }).click();
    await expect(dialog.getByText('150% of fitted size')).toBeVisible();
    await dialog.getByRole('button', { name: 'Fit image' }).click();
    await expect(dialog.getByText('100% of fitted size')).toBeVisible();
    await dialog.getByRole('button', { name: 'Next image' }).click();
    await expect(
      dialog.getByRole('img', { name: 'front.png', exact: true }),
    ).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    await expect(trigger).toBeFocused();
    await page.goto(`/entities/${entity.id}/edit`);
    await page.getByRole('button', { name: 'Remove front.png' }).click();
    await page.getByRole('button', { name: 'Remove attachment' }).click();
    await expect(
      page.getByRole('button', { name: 'Preview front.png' }),
    ).toHaveCount(0);
    await page.reload();
    await expect(
      page.getByRole('button', { name: 'Preview front.png' }),
    ).toHaveCount(0);
    await expect(
      page.getByRole('button', { name: 'Preview back.png' }),
    ).toBeVisible();
  });
}

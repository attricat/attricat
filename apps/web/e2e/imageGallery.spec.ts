import { expect, test } from '@playwright/test';
import {
  createRecord,
  createRecordBlueprint,
  recordSave,
  scalar,
  signInAsMember,
  suffix,
} from './helpers';

// Small valid PNG; the real upload and image worker generate both variants.
const png = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAEAAAAAoCAIAAADBrGu+AAAAPUlEQVR42u3PQQkAAAgEsItuBCMY1Qw+hcEKLNXzWgQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQELhZHmeDxWtWjYgAAAABJRU5ErkJggg==',
  'base64',
);

for (const mode of ['light', 'dark'] as const) {
  test(`image gallery uploads, zooms and edits attachments in ${mode} mode`, async ({
    browser,
    page,
  }, testInfo) => {
    await page.emulateMedia({ colorScheme: mode });
    if (mode === 'dark')
      await page.setViewportSize({ width: 390, height: 844 });
    const blueprint = await createRecordBlueprint(
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
    const record = await createRecord(blueprint, [
      scalar('title', 'Gallery test'),
    ]);
    await page.goto(`/records/${record.id}`);
    const title = page.getByLabel('title', { exact: true });
    // Leaving the edited title for the upload saves it without losing the click.
    await title.fill('Edited title');
    const titleSaved = recordSave(page, record.id);
    await page.locator('input[type="file"]').setInputFiles([
      { name: 'front.png', mimeType: 'image/png', buffer: png },
      { name: 'back.png', mimeType: 'image/png', buffer: png },
    ]);
    await page.getByRole('button', { name: 'Upload 2 files' }).click();
    expect((await titleSaved).ok()).toBe(true);
    await expect(
      page.getByRole('button', { name: 'Preview front.png' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Preview back.png' }),
    ).toBeVisible();
    await expect(title).toHaveValue('Edited title');
    const reordered = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
        `/api/records/${record.id}/file-attributes/photos/references`,
    );
    await page.getByRole('button', { name: 'Move front.png later' }).click();
    expect((await reordered).ok()).toBe(true);
    await expect(
      page.getByRole('button', { name: 'Move front.png earlier' }),
    ).toBeEnabled();
    // Dragging back by the handle restores the original order.
    const dragged = page.waitForResponse(
      (response) =>
        new URL(response.url()).pathname ===
        `/api/records/${record.id}/file-attributes/photos/references`,
    );
    const handle = await page
      .getByRole('button', { name: 'Drag to reorder front.png' })
      .boundingBox();
    const target = await page
      .getByRole('button', { name: 'Preview back.png' })
      .boundingBox();
    if (!handle || !target) throw new Error('gallery tiles are not visible');
    await page.mouse.move(
      handle.x + handle.width / 2,
      handle.y + handle.height / 2,
    );
    await page.mouse.down();
    // Move in steps so the pointer sensor activates and sorting follows.
    await page.mouse.move(handle.x, handle.y + handle.height / 2, {
      steps: 5,
    });
    await page.mouse.move(
      target.x + target.width * 0.25,
      target.y + target.height / 2,
      { steps: 20 },
    );
    await page.mouse.up();
    expect((await dragged).ok()).toBe(true);
    await expect(
      page.getByRole('button', { name: /^Preview .*\.png$/ }).first(),
    ).toHaveAccessibleName('Preview front.png');
    await page.getByRole('button', { name: 'Move front.png later' }).click();
    await expect(
      page.getByRole('button', { name: 'Move front.png earlier' }),
    ).toBeEnabled();
    await page.reload();
    await expect(title).toHaveValue('Edited title');
    const previews = page.getByRole('button', { name: /^Preview .*\.png$/ });
    await expect(previews.first()).toHaveAccessibleName('Preview back.png');
    // Read-only display retains viewing/zoom without exposing editing controls.
    const viewer = await signInAsMember(browser, 'viewer');
    await viewer.emulateMedia({ colorScheme: mode });
    if (mode === 'dark')
      await viewer.setViewportSize({ width: 390, height: 844 });
    await viewer.goto(`/records/${record.id}`);
    await expect(
      viewer.getByRole('button', { name: 'Preview back.png' }),
    ).toBeVisible();
    await expect(
      viewer.getByRole('button', { name: 'Remove front.png' }),
    ).toHaveCount(0);
    const trigger = viewer.getByRole('button', { name: 'Preview back.png' });
    await trigger.focus();
    await viewer.keyboard.press('Enter');
    const dialog = viewer.getByRole('dialog');
    await expect(
      dialog.getByRole('img', { name: 'back.png', exact: true }),
    ).toBeVisible({ timeout: 30_000 });
    await expect(
      dialog.getByRole('img', { name: 'back.png', exact: true }),
    ).toHaveAttribute('src', /\/variants\/display\/download$/);
    await viewer.screenshot({
      path: testInfo.outputPath(`gallery-${mode}.png`),
    });
    await dialog.getByRole('button', { name: 'Zoom in', exact: true }).click();
    await expect(dialog.getByText('150% of fitted size')).toBeVisible();
    await dialog.getByRole('button', { name: 'Fit image' }).click();
    await expect(dialog.getByText('100% of fitted size')).toBeVisible();
    await dialog.getByRole('button', { name: 'Next image' }).click();
    await expect(
      dialog.getByRole('img', { name: 'front.png', exact: true }),
    ).toBeVisible();
    await viewer.keyboard.press('Escape');
    await expect(dialog).toHaveCount(0);
    await expect(trigger).toBeFocused();
    await viewer.context().close();
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

import { expect, test } from '@playwright/test';
import { createBlueprint, suffix } from './helpers';

const extensionId = 'outlet-browser-fixture';
const releaseId = '11111111-1111-4111-8111-111111111111';
const contribution = (id: string, outlet: string) => ({
  contribution_key: `${extensionId}:${id}`,
  display_order: 0,
  navigation_group: null,
  capabilities: [`client.${outlet}`],
  configuration: null,
  extension_id: extensionId,
  extension_name: 'Outlet browser fixture',
  release_id: releaseId,
  id,
  version: 1,
  kind: 'panel',
  outlet,
  route: null,
  title: id,
});

test('mounts a blueprint panel and a publish check only in its confirmation', async ({
  page,
}) => {
  const code = `outlet_${suffix()}`;
  const blueprint = await createBlueprint(`
format_version = 1
code = "${code}"
name = "Outlet fixture"
kind = "record"

[[attributes]]
code = "title"
value_type = "string"

[views.dropdown_option]
type = "dropdown_option"
fields = ["title"]`);
  const blueprintId = blueprint.blueprint.id;
  const version = blueprint.blueprint.version;
  await page.route('**/api/extensions/runtime*', (route) =>
    route.fulfill({
      json: [
        contribution('blueprint-panel-test', 'blueprint_panel'),
        contribution('publish-check-test', 'blueprint_publish_check'),
      ],
    }),
  );
  await page.route(`**/api/extensions/${extensionId}/*/artifact`, (route) =>
    route.fulfill({
      contentType: 'text/javascript',
      body: `export function mount(root, catalog) { root.textContent = JSON.stringify(catalog.context); }`,
    }),
  );

  await page.goto(`/manage/blueprints/${blueprintId}`);
  const panel = page.frameLocator('iframe[title="blueprint-panel-test"]');
  await expect(panel.getByText(blueprintId, { exact: false })).toBeVisible();
  await expect(page.locator('iframe[title="publish-check-test"]')).toHaveCount(
    0,
  );

  await page.getByRole('button', { name: 'Publish', exact: true }).click();
  const check = page.frameLocator('iframe[title="publish-check-test"]');
  await expect(check.getByText(blueprintId, { exact: false })).toBeVisible();
  await expect(
    check.getByText(`"blueprint_version":${version}`, { exact: false }),
  ).toBeVisible();
  await page.getByRole('button', { name: 'Cancel' }).click();
  await expect(page.locator('iframe[title="publish-check-test"]')).toHaveCount(
    0,
  );
});

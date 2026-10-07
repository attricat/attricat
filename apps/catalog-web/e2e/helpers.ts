import {
  expect,
  type Browser,
  type Locator,
  type Page,
} from '@playwright/test';
import { e2eApiUrl, e2eMailpitUrl } from './ports.ts';

const fixtureEmail = 'fixture@example.test';
const fixturePassword = 'e2e-only-fixture-password';

// Fixture setup uses a second seeded principal so login rotation never revokes
// the browser owner's session saved by global setup.
const authenticatedHeaders = async () => {
  const login = await fetch(`${e2eApiUrl}/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      login_identifier: 'default.local',
      email: fixtureEmail,
      password: fixturePassword,
    }),
  });
  if (!login.ok) throw new Error(`E2E fixture login failed: ${login.status}`);
  const cookies = login.headers
    .getSetCookie()
    .map((value) => value.split(';', 1)[0]);
  const csrf = cookies.find((cookie) => cookie.startsWith('catalog_csrf='));
  if (!csrf) throw new Error('E2E fixture login did not issue a CSRF cookie');
  return { cookie: cookies.join('; '), csrf: csrf.split('=', 2)[1] };
};

export type Blueprint = {
  blueprint: { id: string; code: string; version: number };
};
export type Entity = { id: string };
export type Context = { id: string; code: string };
export type NewValue = Record<string, unknown>;

export const suffix = () => crypto.randomUUID().slice(0, 8);

export const request = async <T>(
  path: string,
  init?: RequestInit,
): Promise<T> => {
  const headers = new Headers(init?.headers);
  const auth = await authenticatedHeaders();
  headers.set('Cookie', auth.cookie);
  if (
    !['GET', 'HEAD', 'OPTIONS'].includes((init?.method ?? 'GET').toUpperCase())
  ) {
    headers.set('X-Catalog-Csrf', auth.csrf);
  }
  const response = await fetch(`${e2eApiUrl}${path}`, { ...init, headers });
  if (!response.ok) {
    throw new Error(
      `E2E setup request failed: ${response.status} ${await response.text()}`,
    );
  }
  return response.json() as Promise<T>;
};

export const createBlueprint = (definition: string) =>
  request<Blueprint>('/blueprints', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ definition }),
  });

export const createEntityBlueprint = async (
  code: string,
  name: string,
  attributes: string,
  options: { entitySchema?: string; views?: string } = {},
) => {
  const blueprint = await createBlueprint(
    `format_version = 1\ncode = "${code}"\nname = "${name}"\nkind = "entity"${options.entitySchema ? `\nentity_schema = '${options.entitySchema}'` : ''}\n\n[views.dropdown_option]\ntype = "dropdown_option"\nfields = ["title"]\n\n${attributes}${options.views ? `\n\n${options.views}` : ''}`,
  );
  return publishRevision(blueprint);
};

export const createRevision = (blueprintId: string, definition: string) =>
  request<Blueprint>(`/blueprints/${blueprintId}/versions`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ definition }),
  });

export const publishRevision = (blueprint: Blueprint) =>
  request<Blueprint>(
    `/blueprints/${blueprint.blueprint.id}/versions/${blueprint.blueprint.version}/publish`,
    { method: 'POST' },
  );

export const defaultContext = async () => {
  const contexts = await request<Context[]>('/contexts');
  const context = contexts.find(({ code }) => code === 'default');
  if (!context) throw new Error('E2E setup did not create the default context');
  return context;
};

export const createContext = (code: string, parentId: string, data = {}) =>
  request<Context>('/contexts', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ code, parent_id: parentId, data }),
  });

export const createEntity = async (
  blueprint: Blueprint,
  values: NewValue[] = [],
) => {
  const context = await defaultContext();
  return request<Entity>('/v1/entities', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      blueprint: {
        code: blueprint.blueprint.code,
        version: blueprint.blueprint.version,
      },
      values: values.map((value) => ({ ...value, context_id: context.id })),
    }),
  });
};

export const scalar = (attributeCode: string, value: unknown) => ({
  kind: 'scalar',
  attribute_code: attributeCode,
  value,
});

export const relationship = (
  attributeCode: string,
  targetEntityId: string,
) => ({
  kind: 'relationship',
  attribute_code: attributeCode,
  target_entity_id: targetEntityId,
});

export const appendValues = (entityId: string, values: NewValue[]) =>
  request<void>(`/entities/${entityId}/values`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ values }),
  });

const onboardingUrlFor = async (email: string) => {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    const { messages } = await fetch(`${e2eMailpitUrl}/api/v1/messages`).then(
      (response) => response.json(),
    );
    const message = messages.find(
      (candidate: { To: Array<{ Address: string }> }) =>
        candidate.To.some((recipient) => recipient.Address === email),
    );
    if (message) {
      const delivered = await fetch(
        `${e2eMailpitUrl}/api/v1/message/${message.ID}`,
      ).then((response) => response.json());
      const rawUrl = JSON.stringify(delivered).match(
        /http:\/\/127\.0\.0\.1:\d+\/onboarding\?[^"\\\s]+/,
      )?.[0];
      if (!rawUrl) throw new Error(`Onboarding email for ${email} had no URL`);
      return new URL(
        rawUrl.replaceAll('\\u0026', '&').replaceAll('&amp;', '&'),
      );
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for onboarding email to ${email}`);
};

/** Creates and onboards a workspace member holding one system role. */
export const createMember = async (roleCode: string) => {
  const email = `${roleCode}-${suffix()}@example.test`;
  const password = `${roleCode}-e2e-password`;
  const session = await request<{ workspace_id: string }>('/auth/session');
  const roles = await request<Array<{ id: string; code: string }>>(
    '/workspace/assignable-roles',
  );
  const role = roles.find(({ code }) => code === roleCode);
  if (!role) throw new Error(`E2E workspace has no ${roleCode} role`);
  await request('/workspace/users', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      email,
      role_id: role.id,
      scope_type: 'workspace',
      scope_target_id: session.workspace_id,
      expires_at: new Date(Date.now() + 60 * 60 * 1000).toISOString(),
    }),
  });
  const url = await onboardingUrlFor(email);
  const response = await fetch(`${e2eApiUrl}/onboarding/complete`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      invitation_secret: url.searchParams.get('invitation_secret'),
      onboarding_secret: url.searchParams.get('onboarding_secret'),
      password,
    }),
  });
  if (!response.ok) {
    throw new Error(
      `E2E onboarding failed: ${response.status} ${await response.text()}`,
    );
  }
  return { email, password };
};

/** Replaces the whole document in the page's first Monaco editor. */
export const replaceDefinition = async (page: Page, value: string) => {
  const editor = page.locator('.monaco-editor').first();
  await expect(editor.locator('.view-lines')).toContainText('format_version');
  await editor.locator('.view-lines').click();
  // Monaco's EditContext input inserts text at the caret and ignores the
  // selection, so clear the selected document before inserting.
  await page.keyboard.press('Control+End');
  await page.keyboard.press('Shift+Control+Home');
  await page.keyboard.press('Backspace');
  await page.keyboard.insertText(value);
};

/** Signs a new member holding `roleCode` in through the login page. */
export const signInAsMember = async (browser: Browser, roleCode: string) => {
  const member = await createMember(roleCode);
  const context = await browser.newContext({
    storageState: { cookies: [], origins: [] },
  });
  const page = await context.newPage();
  await page.goto('/login');
  await page.getByLabel('Workspace', { exact: true }).fill('default.local');
  await page.getByRole('button', { name: 'Continue' }).click();
  await page.getByLabel('Email').fill(member.email);
  await page.getByLabel('Password').fill(member.password);
  await page.getByRole('button', { name: 'Sign in' }).click();
  await expect(page).toHaveURL(/\/$/);
  return page;
};

/** Resolves with the entity page's next inline field save of `entityId`. */
export const entitySave = (page: Page, entityId: string) =>
  page.waitForResponse(
    (response) =>
      response.request().method() === 'PUT' &&
      new URL(response.url()).pathname === `/api/v1/entities/${entityId}`,
  );

/**
 * Commits an inline entity field by pressing `key` in it (Tab leaves the
 * field, Enter commits a single-line input) and waits for the save to succeed.
 */
export const commitField = async (
  page: Page,
  entityId: string,
  field: Locator,
  key: 'Tab' | 'Enter' = 'Tab',
) => {
  const saved = entitySave(page, entityId);
  await field.press(key);
  expect((await saved).ok()).toBe(true);
};

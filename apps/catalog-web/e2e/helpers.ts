const apiUrl = 'http://127.0.0.1:43100';

export type Blueprint = {
  blueprint: { id: string; code: string; version: number };
};
export type Entity = { id: string };
export type Context = { id: string; code: string };
export type NewValue = Record<string, unknown>;

export const suffix = () => crypto.randomUUID().slice(0, 8);

export const request = async <T>(path: string, init?: RequestInit): Promise<T> => {
  const response = await fetch(`${apiUrl}${path}`, init);
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
    `format_version = 1\ncode = "${code}"\nname = "${name}"\nkind = "entity"${options.entitySchema ? `\nentity_schema = '${options.entitySchema}'` : ''}\n\n[display.dropdown_option]\nfields = ["title"]\n\n${attributes}${options.views ? `\n\n${options.views}` : ''}`,
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

export const createEntity = async (blueprint: Blueprint, values: NewValue[] = []) => {
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

export const relationship = (attributeCode: string, targetEntityId: string) => ({
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

export type Attribute = {
  code: string;
  value_type: string;
  target_blueprint_code?: string;
  [key: string]: unknown;
};
export type Blueprint = {
  code: string;
  name: string;
  version: number;
  display: Record<string, unknown>;
  [key: string]: unknown;
};
export type BlueprintWithAttributes = {
  blueprint: Blueprint;
  attributes: Attribute[];
};
export type NewAttributeValue =
  | { kind: 'scalar'; attribute_code: string; value: string }
  | {
      kind: 'relationship';
      attribute_code: string;
      target_entity_id: string;
    };
export type RelationshipTargets = {
  attribute_code: string;
  target_entity_ids: string[];
};
export type Entity = {
  id: string;
  [key: string]: unknown;
};
export type EntityFormResponse = {
  entity: Entity;
  blueprint: BlueprintWithAttributes;
  values: NewAttributeValue[];
};
export type EntityItem = {
  id: string;
  display: Record<string, string>;
  preview: Record<string, unknown>;
};
export type EntitySearchResponse = {
  blueprint: { blueprint: Blueprint; attributes: Attribute[] };
  items: EntityItem[];
  next_cursor: string | null;
};

export type RelationshipPreviewItem = {
  id: string;
  display: string;
};
export type RelationshipPreview = {
  items: RelationshipPreviewItem[];
  truncated: boolean;
};
export type EntityPreviewValue =
  | string
  | number
  | boolean
  | null
  | EntityPreviewValue[]
  | { [key: string]: EntityPreviewValue }
  | RelationshipPreview;
export type EntityPreview = Record<string, Record<string, EntityPreviewValue>>;

const request = async <T>(path: string, init?: RequestInit): Promise<T> => {
  const response = await fetch(path, init);
  if (!response.ok) throw new Error(`Request failed (${response.status})`);
  return response.json() as Promise<T>;
};

export const searchEntities = (
  blueprint: string,
  version: number | undefined,
  query: string,
) => {
  return request<EntitySearchResponse>('/api/v1/entities/search', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      blueprint: {
        code: blueprint,
        ...(version === undefined ? {} : { version }),
      },
      query,
      filters: [],
      page: { size: 25, cursor: null },
    }),
  });
};

export const listEntityBlueprints = () => {
  return request<Blueprint[]>('/api/blueprints');
};

export const getEntityPreview = (id: string) => {
  return request<EntityPreview>(
    `/api/entities/${encodeURIComponent(id)}/projections/preview`,
  );
};

export const getBlueprintByCode = (code: string, version?: number) => {
  const path = `/api/blueprints/by-code/${encodeURIComponent(code)}${
    version === undefined ? '' : `/versions/${version}`
  }`;
  return request<BlueprintWithAttributes>(path);
};

export const getEntityForm = (id: string) => {
  return request<EntityFormResponse>(
    `/api/v1/entities/${encodeURIComponent(id)}/form`,
  );
};

export const createEntity = (input: {
  blueprint: { code: string; version?: number };
  values: NewAttributeValue[];
}) => {
  return request<Entity>('/api/v1/entities', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });
};

export const updateEntity = (
  id: string,
  input: {
    values: NewAttributeValue[];
    relationships: RelationshipTargets[];
  },
) => {
  return request<Entity>(`/api/v1/entities/${encodeURIComponent(id)}`, {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(input),
  });
};

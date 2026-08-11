export type ExplorerSearch = {
  blueprint?: string;
  version?: number;
  query?: string;
};

export const parseExplorerSearch = (
  input: Record<string, unknown>,
): ExplorerSearch => {
  const blueprint =
    typeof input.blueprint === 'string' && input.blueprint.trim()
      ? input.blueprint
      : undefined;
  const query =
    typeof input.query === 'string' && input.query.trim()
      ? input.query
      : undefined;
  const versionValue =
    typeof input.version === 'string' ? Number(input.version) : input.version;
  const version =
    typeof versionValue === 'number' &&
    Number.isInteger(versionValue) &&
    versionValue > 0
      ? versionValue
      : undefined;
  return { blueprint, version, query };
};

export const displayLabel = (
  display: Record<string, string> | undefined,
  entityId: string,
): string => {
  return display?.default || entityId;
};

export const dropdownOptionLabel = (
  preview: Record<string, unknown>,
  display: Record<string, unknown>,
): string | undefined => {
  const definition = display.dropdown_option;
  if (!definition || typeof definition !== 'object') return undefined;
  const fields = (definition as Record<string, unknown>).fields;
  if (!Array.isArray(fields)) return undefined;
  const values = preview.default;
  if (!values || typeof values !== 'object') return undefined;
  const separator = (definition as Record<string, unknown>).separator;
  const label = fields
    .filter((field): field is string => typeof field === 'string')
    .map((field) => (values as Record<string, unknown>)[field])
    .filter((value) => value !== null && value !== undefined)
    .map((value) => (typeof value === 'string' ? value : String(value)))
    .join(typeof separator === 'string' ? separator : ' · ');
  return label || undefined;
};

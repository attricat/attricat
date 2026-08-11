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

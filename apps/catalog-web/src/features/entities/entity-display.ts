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

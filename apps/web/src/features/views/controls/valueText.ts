/** A single value as plain text; objects show as their JSON. */
export const valueText = (value: unknown) =>
  typeof value === 'object' ? JSON.stringify(value) : String(value);

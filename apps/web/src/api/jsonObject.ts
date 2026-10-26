/** Whether a parsed JSON value is an object, not `null` or an array. */
export const isJsonObject = (
  value: unknown,
): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

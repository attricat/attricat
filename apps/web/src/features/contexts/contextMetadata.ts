export type ContextMetadataResult =
  | { data: Record<string, unknown>; errorKey?: never }
  | { data?: never; errorKey: string };

/** Parse context metadata entered as JSON; it must be a JSON object. */
export const parseContextMetadata = (value: string): ContextMetadataResult => {
  let data: unknown;
  try {
    data = JSON.parse(value);
  } catch {
    return { errorKey: 'contexts.invalidMetadataJson' };
  }
  if (typeof data !== 'object' || data === null || Array.isArray(data))
    return { errorKey: 'contexts.metadataMustBeObject' };
  return { data: data as Record<string, unknown> };
};

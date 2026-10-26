const uuidByteLength = 16;
const versionByte = 6;
const variantByte = 8;
const uuidV4Version = 0x40;
const rfc4122Variant = 0x80;
const uuidHexGroups = /^(.{8})(.{4})(.{4})(.{4})(.{12})$/;

/**
 * Creates a random UUID v4. Browsers expose `crypto.randomUUID` only in
 * secure contexts, so a page served over plain HTTP from a LAN address builds
 * the UUID from `crypto.getRandomValues`, which every context provides.
 */
export const randomUuid = (): string => {
  if (typeof crypto.randomUUID === 'function') return crypto.randomUUID();
  const bytes = crypto.getRandomValues(new Uint8Array(uuidByteLength));
  bytes[versionByte] = (bytes[versionByte] & 0x0f) | uuidV4Version;
  bytes[variantByte] = (bytes[variantByte] & 0x3f) | rfc4122Variant;
  return [...bytes]
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('')
    .replace(uuidHexGroups, '$1-$2-$3-$4-$5');
};

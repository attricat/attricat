const encoder = new TextEncoder();

/** Size of a string once encoded as UTF-8, as it crosses the frame boundary. */
export const utf8ByteLength = (value: string) => encoder.encode(value).length;

/** Size of a value's JSON serialization in UTF-8 bytes. */
export const jsonByteLength = (value: unknown) =>
  utf8ByteLength(JSON.stringify(value));

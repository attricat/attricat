/** Where the line holding `offset` starts. */
export const lineStart = (text: string, offset: number) =>
  text.lastIndexOf('\n', offset - 1) + 1;

/** Where the line holding `offset` ends, before its line break. */
export const lineEnd = (text: string, offset: number) => {
  const end = text.indexOf('\n', offset);
  return end < 0 ? text.length : end;
};

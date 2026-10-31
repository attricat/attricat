const templateCodeLine = /^code: (\S+)$/m;

/**
 * Renames a template's code to the first of `code`, `code_2`, `code_3`, …
 * that is not taken, so a new draft can be saved as is.
 */
export const withUnusedCode = (
  definition: string,
  takenCodes: ReadonlySet<string>,
) => {
  const base = definition.match(templateCodeLine)?.[1];
  if (!base) return definition;
  let code = base;
  for (let suffix = 2; takenCodes.has(code); suffix += 1)
    code = `${base}_${suffix}`;
  return definition.replace(templateCodeLine, `code: ${code}`);
};
